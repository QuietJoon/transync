//! Optional test-stub `Translator` impl. Compiled in only with the
//! `test-stub` feature so production builds never link it.
//!
//! TRACE: SCN-12 (CLI's `--test-stub-provider` Cargo feature wires this in)
//! TRACE: ADR-0002

use crate::llm::{
    GlossaryEntry, GlossaryExtractionRequest, GlossaryScope, OutputKind, TranslationBatch,
    TranslationBatchResult, Translator, TranslatorError, UnitResult,
};

/// Echoes every unit's `source_payload` back as `OutputKind::Translated`.
///
/// Useful for compile-only smoke tests, the CLI's `test-stub-provider`
/// feature, and the `scripts/smoke.sh` dry path that runs without an API key.
///
/// TRACE: SCN-12
#[derive(Default, Debug)]
pub struct EchoTranslator;

#[async_trait::async_trait]
impl Translator for EchoTranslator {
    async fn translate_batch(
        &self,
        batch: TranslationBatch,
        _cancel: &crate::CancellationToken,
    ) -> Result<TranslationBatchResult, TranslatorError> {
        let units = batch
            .units
            .iter()
            .map(|u| UnitResult {
                unit_id: u.unit_id.clone(),
                output_kind: OutputKind::Translated,
                translated_payload: u.source_payload.clone(),
                warnings: Vec::new(),
            })
            .collect();
        Ok(TranslationBatchResult {
            batch_id: batch.batch_id,
            detected_source_language: None,
            units,
        })
    }
}

/// Echoes like [`EchoTranslator`] and additionally reports
/// `detected_source_language` on every envelope.
///
/// It exists for the `--cache-dir` scenario (DCR-0028 / OI-0017): a second run
/// over the same document must report the *first* run's detection without
/// making a provider call, and proving that needs a stub whose answer can
/// differ between the two runs.
///
/// It deliberately keeps the **defaulted** type-name fingerprint, so two
/// instances configured with different languages share a cache namespace. That
/// is the §1 rule applied, not waived: the rule asks an implementor to
/// distinguish instances that can return different *output* for the same batch,
/// and `language` changes no unit's `translated_payload` — it moves an
/// envelope-level observation about the document, which is exactly the thing
/// the document-metadata record, not the unit key, is responsible for.
///
/// TRACE: DCR-0028
/// TRACE: OI-0017
#[derive(Debug, Default)]
pub struct DetectingEchoTranslator {
    /// Language reported on every envelope. `None` reports nothing, which is
    /// [`EchoTranslator`]'s behavior.
    pub language: Option<String>,
}

#[async_trait::async_trait]
impl Translator for DetectingEchoTranslator {
    async fn translate_batch(
        &self,
        batch: TranslationBatch,
        _cancel: &crate::CancellationToken,
    ) -> Result<TranslationBatchResult, TranslatorError> {
        let units = batch
            .units
            .iter()
            .map(|u| UnitResult {
                unit_id: u.unit_id.clone(),
                output_kind: OutputKind::Translated,
                translated_payload: u.source_payload.clone(),
                warnings: Vec::new(),
            })
            .collect();
        Ok(TranslationBatchResult {
            batch_id: batch.batch_id,
            detected_source_language: self.language.clone(),
            units,
        })
    }
}

/// Echoes like [`EchoTranslator`] and additionally answers the auto-glossary
/// preflight with a fixed harvest.
///
/// It exists for the `--cache-dir` + `--auto-glossary` scenario (ti `dca5bf`):
/// a second run over the same document must **not** call
/// [`Translator::extract_glossary`] at all, and proving that from outside the
/// process needs a stub whose answer can differ between the two runs — a run
/// that asked again and a run that replayed are otherwise indistinguishable in
/// the published artifacts.
///
/// Like [`DetectingEchoTranslator`] it keeps the **defaulted** type-name
/// fingerprint, and for the same reason: the §1 rule asks an implementor to
/// distinguish instances that can return different output *for the same
/// batch*, and `terms` changes no unit's `translated_payload` — a batch
/// already carries the glossary it was built with.
///
/// TRACE: OI-0026
/// TRACE: DCR-0028
#[derive(Debug, Default)]
pub struct ExtractingEchoTranslator {
    /// `(source_term, target_term)` pairs returned as the harvest. An empty
    /// vector is "supported, found nothing", which is distinct from the trait
    /// default's "not supported".
    pub terms: Vec<(String, String)>,
}

#[async_trait::async_trait]
impl Translator for ExtractingEchoTranslator {
    async fn translate_batch(
        &self,
        batch: TranslationBatch,
        _cancel: &crate::CancellationToken,
    ) -> Result<TranslationBatchResult, TranslatorError> {
        let units = batch
            .units
            .iter()
            .map(|u| UnitResult {
                unit_id: u.unit_id.clone(),
                output_kind: OutputKind::Translated,
                translated_payload: u.source_payload.clone(),
                warnings: Vec::new(),
            })
            .collect();
        Ok(TranslationBatchResult {
            batch_id: batch.batch_id,
            detected_source_language: None,
            units,
        })
    }

    async fn extract_glossary(
        &self,
        _req: &GlossaryExtractionRequest,
        _cancel: &crate::CancellationToken,
    ) -> Result<Option<Vec<GlossaryEntry>>, TranslatorError> {
        Ok(Some(
            self.terms
                .iter()
                .map(|(source, target)| GlossaryEntry {
                    source_term: source.clone(),
                    target_term: target.clone(),
                    note: None,
                    scope: GlossaryScope::GlobalAcrossDocument,
                    sections: Vec::new(),
                })
                .collect(),
        ))
    }
}

/// Fails every batch with a caller-chosen **terminal** [`TranslatorError`],
/// so a whole-run abort (ADR-0017) can be driven from outside the process
/// with no live provider.
///
/// It exists for the CLI's exit-code contract (ti `e62b59`): once
/// `TranslatorError` names its terminal causes (DCR-0023 / DCR-0029), the CLI
/// answers different causes with different process exit codes, and a mapping
/// function tested in isolation proves only half of that — the half that does
/// not include the pipeline actually surfacing the error, the CLI actually
/// classifying it, and the process actually exiting with the number. Pinning
/// the other half needs a translator that can fail *on purpose*, one cause at
/// a time.
///
/// It carries a **factory** rather than an error value because
/// [`TranslatorError`] is deliberately not `Clone` — every variant wraps an
/// owned diagnostic string — and `translate_batch` takes `&self`, so there is
/// nothing to move out. Building a fresh error per call is also the honest
/// shape: a provider that fails twice produces two errors.
///
/// TRACE: ti e62b59
pub struct TerminalErrorTranslator {
    make: Box<dyn Fn() -> TranslatorError + Send + Sync>,
}

impl TerminalErrorTranslator {
    /// Build a stub that answers every `translate_batch` call with
    /// `make()`.
    pub fn new(make: impl Fn() -> TranslatorError + Send + Sync + 'static) -> Self {
        Self {
            make: Box::new(make),
        }
    }
}

impl std::fmt::Debug for TerminalErrorTranslator {
    /// Hand-written because a boxed closure has no `Debug`. It names the
    /// error the stub is configured to raise, which is the only thing about
    /// it worth printing.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TerminalErrorTranslator")
            .field("error", &(self.make)())
            .finish()
    }
}

#[async_trait::async_trait]
impl Translator for TerminalErrorTranslator {
    async fn translate_batch(
        &self,
        _batch: TranslationBatch,
        _cancel: &crate::CancellationToken,
    ) -> Result<TranslationBatchResult, TranslatorError> {
        Err((self.make)())
    }
}

/// Always-fails stub: returns `OutputKind::FailedNeedsFallback` for every
/// unit. Used by the CLI's exit-code-3 smoke test (`R0001-0086` in the
/// removed `reviews/reviewed/0001.md`) to drive the all-units-fell-back path
/// without a live provider.
///
/// TRACE: SCN-08
#[derive(Default, Debug)]
pub struct AlwaysFailsTranslator;

#[async_trait::async_trait]
impl Translator for AlwaysFailsTranslator {
    async fn translate_batch(
        &self,
        batch: TranslationBatch,
        _cancel: &crate::CancellationToken,
    ) -> Result<TranslationBatchResult, TranslatorError> {
        let units = batch
            .units
            .iter()
            .map(|u| UnitResult {
                unit_id: u.unit_id.clone(),
                output_kind: OutputKind::FailedNeedsFallback,
                translated_payload: String::new(),
                warnings: Vec::new(),
            })
            .collect();
        Ok(TranslationBatchResult {
            batch_id: batch.batch_id,
            detected_source_language: None,
            units,
        })
    }
}
