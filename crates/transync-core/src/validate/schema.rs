//! Schema layer: ID-set equality + duplicate / extraneous unit detection,
//! plus the one payload-byte rule the JSON schema cannot express.
//!
//! Two envelope entry points over one classification:
//! - [`classify`] decomposes a batch result **per unit** — which requested
//!   ids the provider dropped, which it returned more than once, which ids
//!   it invented, and whether the *request* itself was malformed. This is
//!   what [`crate::validate::validate_batch`] uses so a batch-level fault
//!   can be attributed to the units actually at fault instead of blanket-
//!   rejecting every batch-mate (OI-0031).
//! - `check_schema` is the legacy pass/fail form. OI-0027 made `validate`
//!   crate-private, so it has no external callers left and is now
//!   test-only — retained as the oracle that pins [`classify`]'s
//!   first-error strings and precedence against regression.
//!
//! …and one **per-unit** entry point, [`check_payload_bytes`], which judges
//! a single accepted payload's bytes rather than the envelope around it. It
//! lives here because a JSON string may legally hold a `U+0000` and
//! `out.md` may not, so the rule is the part of "the response is a
//! well-formed payload" that Structured Output has no way to carry
//! (ti d06c43).
//!
//! TRACE: SCN-01
//! TRACE: SCN-09
//! TRACE: OI-0031
//! TRACE: OI-0034

use crate::id::BlockId;
use crate::llm::{InputMode, TranslationBatch, TranslationBatchResult};
use std::collections::HashMap;
// Only `check_schema` (test-only since OI-0027) needs set semantics.
#[cfg(test)]
use std::collections::HashSet;

/// How many ids a diagnostic names before it summarizes the remainder.
/// R0006-0039: the operator needs to see WHICH units are implicated, but
/// a 400-unit batch must not produce a 400-id error string.
const ID_LIST_CAP: usize = 8;

/// Per-unit decomposition of the schema layer (OI-0031).
///
/// Every vector is sorted by id string, so reasons, report rows, and
/// retry ordering are deterministic regardless of provider response order.
///
/// The four classes are deliberately distinct because they are charged
/// differently: `missing` and `duplicated` are re-dispatchable *offenders*
/// (charged to the per-batch schema budget), `foreign` rows are discarded
/// and charged to nobody, and `request_duplicates` is a caller-side
/// contract violation that no amount of re-dispatching can fix.
///
/// TRACE: SCN-01
/// TRACE: OI-0031
#[derive(Debug, Clone, Default)]
pub struct SchemaClassification {
    /// unit_ids appearing more than once in the REQUEST (caller bug — the
    /// pipeline's own batcher can never produce one).
    pub request_duplicates: Vec<BlockId>,
    /// Requested ids with no result row (the provider dropped them).
    pub missing: Vec<BlockId>,
    /// Requested ids with more than one result row. Every copy is
    /// discarded: with no principled way to pick one, trusting either
    /// would be a coin flip on content.
    pub duplicated: Vec<BlockId>,
    /// Result ids that were never requested (discarded).
    pub foreign: Vec<String>,
}

impl SchemaClassification {
    /// True when the result maps 1:1 onto the request.
    pub fn is_clean(&self) -> bool {
        self.request_duplicates.is_empty()
            && self.missing.is_empty()
            && self.duplicated.is_empty()
            && self.foreign.is_empty()
    }

    /// Requested units to re-dispatch: those whose content was never
    /// judged (`missing`) or whose copies were all discarded
    /// (`duplicated`). Sorted, missing first.
    pub fn offenders(&self) -> impl Iterator<Item = &BlockId> {
        self.missing.iter().chain(self.duplicated.iter())
    }

    /// Human-readable summary of every non-empty class, for
    /// [`crate::validate::BatchFault::reason`] and the pipeline's warning.
    pub fn summary(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        if !self.request_duplicates.is_empty() {
            parts.push(format!(
                "duplicate unit_id in request: {}",
                id_list(self.request_duplicates.iter().map(|id| id.0.as_str()))
            ));
        }
        if !self.missing.is_empty() {
            parts.push(format!(
                "{} missing from result: {}",
                self.missing.len(),
                id_list(self.missing.iter().map(|id| id.0.as_str()))
            ));
        }
        if !self.duplicated.is_empty() {
            parts.push(format!(
                "{} duplicated in result: {}",
                self.duplicated.len(),
                id_list(self.duplicated.iter().map(|id| id.0.as_str()))
            ));
        }
        if !self.foreign.is_empty() {
            parts.push(format!(
                "{} unrequested in result: {}",
                self.foreign.len(),
                id_list(self.foreign.iter().map(|s| s.as_str()))
            ));
        }
        if parts.is_empty() {
            return "no schema fault".to_string();
        }
        parts.join("; ")
    }
}

/// Decompose a batch result against its request (OI-0031).
///
/// Total and non-failing: it reports every class it finds rather than
/// stopping at the first, which is what lets the caller separate innocent
/// units (exactly one result row) from the ones actually at fault.
///
/// TRACE: SCN-01
/// TRACE: OI-0031
pub fn classify(batch: &TranslationBatch, result: &TranslationBatchResult) -> SchemaClassification {
    let mut cls = SchemaClassification::default();

    // R0008-0022: a malformed caller-built batch with duplicate unit IDs
    // would be silently collapsed by the `requested` set below, so name
    // the offending ids instead of losing them.
    let mut request_counts: HashMap<&BlockId, usize> = HashMap::new();
    for u in &batch.units {
        *request_counts.entry(&u.unit_id).or_insert(0) += 1;
    }
    cls.request_duplicates = request_counts
        .iter()
        .filter(|(_, n)| **n > 1)
        .map(|(id, _)| (*id).clone())
        .collect();

    let mut returned_counts: HashMap<&BlockId, usize> = HashMap::new();
    for r in &result.units {
        if request_counts.contains_key(&r.unit_id) {
            *returned_counts.entry(&r.unit_id).or_insert(0) += 1;
        } else {
            cls.foreign.push(r.unit_id.0.clone());
        }
    }
    for id in request_counts.keys() {
        match returned_counts.get(*id).copied().unwrap_or(0) {
            0 => cls.missing.push((*id).clone()),
            1 => {}
            _ => cls.duplicated.push((*id).clone()),
        }
    }

    // `BlockId` intentionally carries no `Ord` (ADR-0005 keeps it an opaque
    // wire string), so sort through the inner string.
    cls.request_duplicates.sort_by(|a, b| a.0.cmp(&b.0));
    cls.missing.sort_by(|a, b| a.0.cmp(&b.0));
    cls.duplicated.sort_by(|a, b| a.0.cmp(&b.0));
    cls.foreign.sort();
    cls
}

/// Validate the schema layer: every requested unit has exactly one
/// response, no extras, no duplicates.
///
/// Legacy pass/fail form of [`classify`], preserved verbatim — same
/// signature, same first-error message strings, same precedence
/// (malformed request → the first offending result row in provider order
/// → missing ids). [`crate::validate::validate_batch`] uses [`classify`]
/// instead so it can spare the innocent batch-mates.
///
/// Test-only since OI-0027 made `validate` crate-private: no external
/// caller can reach it, and in-crate the only consumer is
/// `check_schema_first_error_strings_are_unchanged`, which is exactly the
/// point — it holds `classify`'s diagnostics byte-compatible with the
/// pre-OI-0031 wording.
///
/// TRACE: SCN-01
#[cfg(test)]
pub fn check_schema(
    batch: &TranslationBatch,
    result: &TranslationBatchResult,
) -> Result<(), String> {
    let requested: HashSet<&BlockId> = batch.units.iter().map(|u| &u.unit_id).collect();
    if requested.len() != batch.units.len() {
        return Err(request_duplicate_message(
            batch.units.len(),
            requested.len(),
        ));
    }
    // Result-row faults are reported in the order the provider returned
    // them, which is the pre-OI-0031 behavior this entry point pins.
    let mut returned: HashSet<&BlockId> = HashSet::new();
    for r in &result.units {
        if !returned.insert(&r.unit_id) {
            return Err(format!("duplicate unit_id in result: {}", r.unit_id));
        }
        if !requested.contains(&r.unit_id) {
            return Err(format!("unexpected unit_id in result: {}", r.unit_id));
        }
    }
    if returned.len() != requested.len() {
        // R0006-0039: name the missing IDs (capped) so the operator can
        // see WHICH units the provider dropped, not just how many.
        let mut missing: Vec<&str> = requested
            .iter()
            .filter(|id| !returned.contains(*id))
            .map(|id| id.0.as_str())
            .collect();
        missing.sort_unstable();
        return Err(format!(
            "unit count mismatch: requested {}, returned {}; missing: {}",
            requested.len(),
            returned.len(),
            id_list(missing.into_iter()),
        ));
    }
    Ok(())
}

/// Stable opening of every payload-byte rejection reason, so a report
/// consumer can recognize the class without matching the whole sentence
/// (the html arm appends which segment carried the byte).
pub(crate) const NUL_IN_PAYLOAD: &str = "translated payload contains a NUL byte (U+0000)";

/// Reject a translated payload carrying a NUL byte (`U+0000`) — the one
/// byte `out.md` never contains.
///
/// The source side has been NUL-free since OI-0034: `markdown::parse`
/// substitutes U+FFFD before comrak sees the document, because CommonMark
/// §2.3 requires the substitution and comrak performs it before it reports
/// a single source position. The *target* side had no such gate.
/// `regen::regenerate` splices the provider's bytes verbatim, so a NUL in a
/// payload reached `out.md` intact while the rendered target pane — which
/// goes through comrak — showed U+FFFD for the same byte: two outputs of
/// one run disagreeing about a character the document contains (ti d06c43).
///
/// The posture is **fail the unit**, not normalize it (owner-delegated
/// decision, 2026-08-07). Substituting inside validation would change what
/// "the payload bytes the provider returned" means for the [`crate::cache`]
/// and for the [`crate::validate::ValidationReport`], which is exactly why
/// ti 743d27 left this out of the source-side fix. Rejecting keeps both
/// honest and routes the unit down the ordinary retry-then-fallback path
/// (ADR-0009 verbatim resubmission, then `fallback_source` — whose bytes
/// are the block's source bytes, NUL-free by construction). A NUL in
/// translated prose is pathological, so the fallback is a cost nothing
/// real pays.
///
/// It has to run first, ahead of every content layer. The shape layers
/// cannot see the byte at all — they read the payload through comrak, which
/// has already replaced it by the time a node exists — and the one check
/// that would notice, the `Preserved` byte-for-byte proof, would blame
/// payload fidelity for a fault that is not about fidelity.
///
/// TRACE: SCN-07
/// TRACE: OI-0034
pub(crate) fn check_payload_bytes(input_mode: &InputMode, payload: &str) -> Result<(), String> {
    // An html unit's payload is a JSON array of text segments, and regen
    // splices the *decoded* segments — so a NUL rides in escaped
    // (as the six characters `\u0000`), invisible to a scan of the
    // payload text. Decode first.
    // A payload that will not decode is malformed JSON, which
    // `per_kind::check_html` rejects a moment later with a better
    // diagnostic; fall through to the raw scan so no path is left uncovered
    // (a payload holding a *literal* NUL is not valid JSON either).
    //
    // ti 490d97 wave 2: keyed on the MODE, which is what makes the decode
    // question answerable at all — the payload shape is the mode's statement,
    // not the kind's, and after the HTML intake lands the two disagree.
    if matches!(input_mode, InputMode::HtmlSegments)
        && let Ok(segments) = serde_json::from_str::<Vec<String>>(payload)
    {
        return match segments.iter().position(|s| s.contains('\0')) {
            Some(i) => Err(format!("{NUL_IN_PAYLOAD} in html segment {i}")),
            None => Ok(()),
        };
    }
    if payload.contains('\0') {
        return Err(NUL_IN_PAYLOAD.to_string());
    }
    Ok(())
}

/// The caller-malformed-request diagnostic, shared by `check_schema` and
/// the defensive `validate_batch` path so the two never drift.
pub(crate) fn request_duplicate_message(total_units: usize, unique_ids: usize) -> String {
    format!("duplicate unit_id in request: {total_units} units but {unique_ids} unique IDs")
}

/// Render a bracketed id list capped at [`ID_LIST_CAP`], with the
/// remainder summarized as a `(+N more)` tail *outside* the brackets —
/// the exact shape the pre-OI-0031 missing-ids diagnostic used. The
/// iterator must already be in its final (sorted) order.
fn id_list<'a>(ids: impl Iterator<Item = &'a str>) -> String {
    let all: Vec<&str> = ids.collect();
    let shown = all.iter().take(ID_LIST_CAP).copied().collect::<Vec<_>>();
    let suffix = if all.len() > ID_LIST_CAP {
        format!(" (+{} more)", all.len() - ID_LIST_CAP)
    } else {
        String::new()
    };
    format!("[{}]{}", shown.join(", "), suffix)
}

// OI-0031: `classify` is the attribution primitive the pipeline's budget
// split rests on, and `check_schema` must stay byte-compatible with its
// pre-OI-0031 messages for external callers.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::BlockKind;
    use crate::llm::{
        BatchId, BlockConstraints, BlockContext, InputMode, OutputKind, TranslationUnit, UnitResult,
    };
    use crate::profile::default_profile;

    fn unit(id: &str) -> TranslationUnit {
        TranslationUnit {
            unit_id: BlockId(id.to_string()),
            block_kind: BlockKind::Paragraph,
            input_mode: InputMode::TextFragment,
            source_payload: format!("source of {id}"),
            context: BlockContext::default(),
            constraints: BlockConstraints::default(),
            source_hash: 0,
            batch_id: BatchId::new(1),
            retry: None,
        }
    }

    fn batch(ids: &[&str]) -> TranslationBatch {
        TranslationBatch {
            batch_id: BatchId::new(1),
            units: ids.iter().map(|id| unit(id)).collect(),
            source_language: "en".to_string(),
            target_language: "ko".to_string(),
            glossary: Vec::new(),
            profile: default_profile(),
        }
    }

    fn result(ids: &[&str]) -> TranslationBatchResult {
        TranslationBatchResult {
            batch_id: BatchId::new(1),
            detected_source_language: None,
            units: ids
                .iter()
                .map(|id| UnitResult {
                    unit_id: BlockId(id.to_string()),
                    output_kind: OutputKind::Translated,
                    translated_payload: format!("번역 {id}"),
                    warnings: Vec::new(),
                })
                .collect(),
        }
    }

    fn ids(v: &[BlockId]) -> Vec<&str> {
        v.iter().map(|id| id.0.as_str()).collect()
    }

    #[test]
    fn clean_result_is_clean() {
        let cls = classify(
            &batch(&["p-0001", "p-0002"]),
            &result(&["p-0002", "p-0001"]),
        );
        assert!(cls.is_clean(), "{cls:?}");
        assert_eq!(cls.offenders().count(), 0);
        assert_eq!(cls.summary(), "no schema fault");
        assert!(
            check_schema(
                &batch(&["p-0001", "p-0002"]),
                &result(&["p-0002", "p-0001"])
            )
            .is_ok()
        );
    }

    #[test]
    fn missing_ids_are_classified_and_sorted() {
        let cls = classify(
            &batch(&["p-0003", "p-0001", "p-0002"]),
            &result(&["p-0002"]),
        );
        assert_eq!(ids(&cls.missing), vec!["p-0001", "p-0003"]);
        assert!(cls.duplicated.is_empty() && cls.foreign.is_empty());
        assert_eq!(
            ids(&cls.offenders().cloned().collect::<Vec<_>>()),
            vec!["p-0001", "p-0003"]
        );
        assert!(
            cls.summary()
                .contains("2 missing from result: [p-0001, p-0003]")
        );
    }

    #[test]
    fn duplicated_result_rows_are_classified_not_missing() {
        let cls = classify(
            &batch(&["p-0001", "p-0002"]),
            &result(&["p-0001", "p-0001", "p-0002"]),
        );
        assert_eq!(ids(&cls.duplicated), vec!["p-0001"]);
        assert!(cls.missing.is_empty(), "{cls:?}");
        assert_eq!(cls.offenders().count(), 1);
    }

    #[test]
    fn foreign_ids_are_classified_without_offenders() {
        let cls = classify(&batch(&["p-0001"]), &result(&["p-0001", "x-9999"]));
        assert_eq!(cls.foreign, vec!["x-9999".to_string()]);
        assert_eq!(
            cls.offenders().count(),
            0,
            "an unrequested id implicates no requested unit"
        );
        assert!(!cls.is_clean());
    }

    #[test]
    fn missing_and_foreign_combine() {
        let cls = classify(
            &batch(&["p-0001", "p-0002"]),
            &result(&["p-0001", "zz-0001"]),
        );
        assert_eq!(ids(&cls.missing), vec!["p-0002"]);
        assert_eq!(cls.foreign, vec!["zz-0001".to_string()]);
        let s = cls.summary();
        assert!(
            s.contains("missing from result") && s.contains("unrequested in result"),
            "{s}"
        );
    }

    #[test]
    fn request_duplicates_are_named() {
        let cls = classify(&batch(&["p-0001", "p-0001"]), &result(&["p-0001"]));
        assert_eq!(ids(&cls.request_duplicates), vec!["p-0001"]);
        assert!(!cls.is_clean());
    }

    // Message parity with the pre-OI-0031 implementation, class by class.
    #[test]
    fn check_schema_first_error_strings_are_unchanged() {
        let err = check_schema(&batch(&["p-0001", "p-0001"]), &result(&["p-0001"]))
            .expect_err("malformed request rejects");
        assert_eq!(
            err,
            "duplicate unit_id in request: 2 units but 1 unique IDs"
        );

        let err = check_schema(&batch(&["p-0001"]), &result(&["p-0001", "p-0001"]))
            .expect_err("duplicate result row rejects");
        assert_eq!(err, "duplicate unit_id in result: p-0001");

        let err = check_schema(&batch(&["p-0001"]), &result(&["p-0001", "x-0001"]))
            .expect_err("foreign result row rejects");
        assert_eq!(err, "unexpected unit_id in result: x-0001");

        let err = check_schema(&batch(&["p-0001", "p-0002"]), &result(&["p-0002"]))
            .expect_err("missing row rejects");
        assert_eq!(
            err,
            "unit count mismatch: requested 2, returned 1; missing: [p-0001]"
        );
    }

    // The capped list keeps a huge drop diagnosable without dumping every id.
    #[test]
    fn missing_id_list_is_capped() {
        let owned: Vec<String> = (1..=12).map(|i| format!("p-{i:04}")).collect();
        let all: Vec<&str> = owned.iter().map(String::as_str).collect();
        let err = check_schema(&batch(&all), &result(&[])).expect_err("all rows missing");
        assert!(err.contains("(+4 more)"), "{err}");
        assert!(err.contains("p-0001, p-0002"), "{err}");
        assert!(!err.contains("p-0012"), "capped list must stop at 8: {err}");
    }

    // ti d06c43: the payload-byte rule. Fixtures are inline `&str` for the
    // same reason OI-0034's are — a NUL is what an editor or a filter drops
    // on the way to a checked-in file, so a checked-in fixture cannot be
    // trusted to still carry one.
    const NUL: char = '\0';

    #[test]
    fn a_markdown_payload_carrying_a_nul_is_rejected() {
        let payload = format!("안녕{NUL}하세요.");
        let err = check_payload_bytes(&InputMode::TextFragment, &payload)
            .expect_err("a NUL in a markdown payload must reject");
        assert_eq!(err, NUL_IN_PAYLOAD);
    }

    #[test]
    fn a_nul_free_payload_passes_in_every_input_mode() {
        for mode in [
            InputMode::TextFragment,
            InputMode::FullTableMarkdown,
            InputMode::FullCodeBlock {
                language_info: None,
            },
            InputMode::ListItemContent,
            InputMode::BlockquoteContent,
            InputMode::HtmlSegments,
            // The seventh variant. A window's payload is a whole GFM table
            // and takes the same raw scan `FullTableMarkdown` does, so
            // omitting it would leave the test's name claiming a totality
            // its body does not have -- the exact name-vs-body drift this
            // repository's tests are policed for.
            InputMode::TableRowWindow {
                parent_block_id: crate::id::BlockId("t-0001".to_string()),
                window_index: 0,
                window_count: 2,
            },
        ] {
            // The html arm reads its payload as a segment array; the others
            // read raw markdown. Give each the shape it expects so the pass
            // is a real pass and not a decode failure.
            let payload = if matches!(mode, InputMode::HtmlSegments) {
                "[\"하나\", \"둘\"]"
            } else {
                "표준 문단, U+FFFD도 아니고 NUL도 아님."
            };
            assert!(
                check_payload_bytes(&mode, payload).is_ok(),
                "{mode:?} rejected a clean payload"
            );
        }
    }

    /// The discriminating case: an html payload is a JSON array, so a NUL
    /// arrives as the six characters `\u0000` and a scan of the payload
    /// *text* sees nothing. regen splices the decoded segments, so the
    /// check has to decode too.
    #[test]
    fn an_escaped_nul_inside_an_html_segment_is_rejected() {
        let payload = r#"["하나", "둘\u0000셋"]"#;
        assert!(
            !payload.contains('\0'),
            "the fixture must carry the ESCAPE, not the byte — otherwise \
             this test would pass through the raw scan and prove nothing"
        );
        let err = check_payload_bytes(&InputMode::HtmlSegments, payload)
            .expect_err("a decoded NUL segment must reject");
        assert_eq!(err, format!("{NUL_IN_PAYLOAD} in html segment 1"));
    }

    /// An html payload that will not decode still gets the raw scan: a
    /// literal NUL is not valid JSON, so the decode arm cannot be the only
    /// thing standing between it and `out.md`.
    #[test]
    fn a_literal_nul_in_an_undecodable_html_payload_is_still_rejected() {
        let payload = format!("[\"하{NUL}나\"]");
        assert!(
            serde_json::from_str::<Vec<String>>(&payload).is_err(),
            "a raw control character inside a JSON string is malformed JSON, \
             which is what routes this fixture to the raw scan"
        );
        let err = check_payload_bytes(&InputMode::HtmlSegments, &payload)
            .expect_err("the raw scan must catch what the decode could not");
        assert_eq!(err, NUL_IN_PAYLOAD);
    }
}
