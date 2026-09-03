//! In-process `Translator` mock used by the scenario tests.
//!
//! Modes mirror `implementation-slice-checklists.md` — each SL slice that
//! exercises a mock variant will switch the `mode` field.
//!
//! TRACE: SCN-07
//! TRACE: SCN-08
//! TRACE: SCN-11

use std::sync::Mutex;
use transync::BlockId;
use transync::llm::{
    OutputKind, TranslationBatch, TranslationBatchResult, Translator, TranslatorError, UnitResult,
};

/// Test-only translator with selectable behavior.
///
/// TRACE: SCN-07
pub struct MockTranslator {
    pub mode: Mode,
    /// Counter shared across calls; used by `RejectsThenAccepts`.
    pub call_count: Mutex<u32>,
    /// Every batch passed to `translate_batch`. Used by `Recording`.
    pub recorded_batches: Mutex<Vec<TranslationBatch>>,
}

/// Behavior selector for [`MockTranslator`].
///
/// TRACE: SCN-07
#[derive(Debug, Clone)]
pub enum Mode {
    /// Echo `source_payload` back as `Translated` for every unit.
    ///
    /// TRACE: SCN-01..SCN-06, SCN-09, SCN-10, SCN-12, SCN-14
    Passthrough,

    /// First call: emit a structurally invalid result (truncated payloads)
    /// for every unit. Subsequent calls: behave as Passthrough.
    ///
    /// TRACE: SCN-07
    RejectsThenAccepts,

    /// Always emit `FailedNeedsFallback` for the unit whose ID matches.
    ///
    /// TRACE: SCN-08
    AlwaysFailsUnit(BlockId),

    /// Pass-through behavior, but record every batch for later inspection.
    ///
    /// TRACE: SCN-09
    Recording,

    /// Always emit `FailedNeedsFallback` for every unit. Drives the SCN-12
    /// exit-code-3 test.
    ///
    /// TRACE: SCN-12
    AlwaysFailsAll,

    /// Pass-through behavior plus a fixed `detected_source_language`.
    ///
    /// TRACE: SCN-11
    EchoesDetectedLanguage(String),

    /// Pass-through, but deterministically rewrites every occurrence of
    /// `from` to `to` in each unit's payload before returning it as
    /// `Translated`. Drives the inline-protection scenarios: a tampered
    /// link destination is caught by `ValidationLayer::Inline`; the same
    /// rewrite is accepted when the profile localizes URLs.
    ///
    /// TRACE: EXT-2026-07 P1-5
    TamperPayload { from: String, to: String },

    /// Echo every unit, except: the matching unit's segment-array payload
    /// gains one appended segment ("EXTRA") — on EVERY attempt, so the
    /// per-kind count check rejects each retry identically and the unit
    /// exhausts into FallbackSource. Drives SCN-16's invariant-6 leg with a
    /// real layer-2 rejection rather than a provider-declared failure.
    ///
    /// TRACE: SCN-16
    AppendsExtraSegment(BlockId),
}

impl MockTranslator {
    pub fn passthrough() -> Self {
        Self::new(Mode::Passthrough)
    }

    pub fn rejects_then_accepts() -> Self {
        Self::new(Mode::RejectsThenAccepts)
    }

    pub fn always_fails_unit(unit_id: BlockId) -> Self {
        Self::new(Mode::AlwaysFailsUnit(unit_id))
    }

    pub fn recording() -> Self {
        Self::new(Mode::Recording)
    }

    pub fn always_fails_all() -> Self {
        Self::new(Mode::AlwaysFailsAll)
    }

    pub fn echoes_detected(lang: &str) -> Self {
        Self::new(Mode::EchoesDetectedLanguage(lang.to_string()))
    }

    pub fn tamper_payload(from: &str, to: &str) -> Self {
        Self::new(Mode::TamperPayload {
            from: from.to_string(),
            to: to.to_string(),
        })
    }

    pub fn appends_extra_segment(unit_id: BlockId) -> Self {
        Self::new(Mode::AppendsExtraSegment(unit_id))
    }

    fn new(mode: Mode) -> Self {
        Self {
            mode,
            call_count: Mutex::new(0),
            recorded_batches: Mutex::new(Vec::new()),
        }
    }

    pub fn call_count(&self) -> u32 {
        *self.call_count.lock().unwrap()
    }

    pub fn recorded(&self) -> Vec<TranslationBatch> {
        self.recorded_batches.lock().unwrap().clone()
    }
}

#[async_trait::async_trait]
impl Translator for MockTranslator {
    async fn translate_batch(
        &self,
        batch: TranslationBatch,
        _cancel: &transync::CancellationToken,
    ) -> Result<TranslationBatchResult, TranslatorError> {
        let attempt = {
            let mut c = self.call_count.lock().unwrap();
            *c += 1;
            *c
        };

        if matches!(self.mode, Mode::Recording) {
            self.recorded_batches.lock().unwrap().push(batch.clone());
        }

        let detected_source_language = match &self.mode {
            Mode::EchoesDetectedLanguage(lang) => Some(lang.clone()),
            _ => None,
        };

        let units = batch
            .units
            .iter()
            .map(|u| {
                let output_kind = match &self.mode {
                    Mode::AlwaysFailsAll => OutputKind::FailedNeedsFallback,
                    Mode::AlwaysFailsUnit(target) if &u.unit_id == target => {
                        OutputKind::FailedNeedsFallback
                    }
                    Mode::RejectsThenAccepts if attempt == 1 => OutputKind::PartiallyTranslated,
                    _ => OutputKind::Translated,
                };

                let translated_payload = match output_kind {
                    OutputKind::FailedNeedsFallback => String::new(),
                    OutputKind::PartiallyTranslated => {
                        // Truncate to force per-kind validation rejection (SL-07 fills in).
                        u.source_payload.chars().take(8).collect()
                    }
                    // TamperPayload rewrites the payload deterministically;
                    // every other translating mode echoes the source verbatim.
                    _ => match &self.mode {
                        Mode::TamperPayload { from, to } => {
                            u.source_payload.replace(from.as_str(), to.as_str())
                        }
                        Mode::AppendsExtraSegment(target) if &u.unit_id == target => {
                            let mut segs: Vec<String> = serde_json::from_str(&u.source_payload)
                                .expect("an html unit's payload is a segment array");
                            segs.push("EXTRA".to_string());
                            serde_json::to_string(&segs).expect("serializes")
                        }
                        _ => u.source_payload.clone(),
                    },
                };

                UnitResult {
                    unit_id: u.unit_id.clone(),
                    output_kind,
                    translated_payload,
                    warnings: Vec::new(),
                }
            })
            .collect();

        Ok(TranslationBatchResult {
            batch_id: batch.batch_id,
            detected_source_language,
            units,
        })
    }
}
