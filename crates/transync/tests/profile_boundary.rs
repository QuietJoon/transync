//! `[[glossary]].scope = "section"` reaches the model in its own sections and
//! nowhere else.
//!
//! This file used to pin the opposite. `profile::load_profile` rejected the
//! reserved `conditional-on-section` scope, and because `ProfileMetadata` is
//! `Deserialize` with public fields — a profile assembled from JSON, or by
//! pushing an entry onto `default_profile()`, never crosses the loader — the
//! translate boundary raised the same refusal (R0001-0006). The refusal was
//! honest but temporary: with a section-blind batcher, honoring the scope
//! would have meant rendering a section's term into every batch's system
//! prompt, steering the whole document (ADR-0014, amended 2026-07-13).
//!
//! DCR-0027 supplied what the scope was missing — a `sections` selector, and a
//! packer whose batches belong to one section each — so the gates are gone and
//! these tests are their acceptance twins: the same profiles now translate,
//! and a recording translator proves which system prompt each section's batch
//! actually carried.
//!
//! TRACE: ADR-0014
//! TRACE: DCR-0027

use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use transync::llm::{
    GlossaryExtractionRequest, OutputKind, TranslationBatch, TranslationBatchResult, Translator,
    TranslatorError, UnitResult,
};
use transync::profile::default_profile;
use transync::{
    GlossaryEntry, GlossaryScope, ProfileMetadata, TranslateOptions, profile, translate,
};

/// Four sections: a preamble that no heading encloses, `# Guide`, and two
/// subsections under it.
const SOURCE: &str = "preamble prose about a cell\n\n\
     # Guide\n\nguide prose about a cell\n\n\
     ## Tables\n\ntable prose about a cell\n\n\
     ## Prisons\n\nprison prose about a cell\n";

/// Echoes every unit back and records the system prompt each batch carried,
/// keyed by the unit ids in it — the only way to see, from outside the crate,
/// which glossary a section was translated under.
#[derive(Default)]
struct RecordingTranslator {
    batch_calls: AtomicUsize,
    extraction_calls: AtomicUsize,
    prompts: Mutex<Vec<(Vec<String>, String)>>,
}

impl RecordingTranslator {
    /// The system prompt of the batch holding `unit_id`.
    fn prompt_for(&self, unit_id: &str) -> String {
        self.prompts
            .lock()
            .unwrap()
            .iter()
            .find(|(ids, _)| ids.iter().any(|i| i == unit_id))
            .map(|(_, prompt)| prompt.clone())
            .unwrap_or_else(|| panic!("no batch carried {unit_id}"))
    }
}

#[async_trait::async_trait]
impl Translator for RecordingTranslator {
    async fn translate_batch(
        &self,
        batch: TranslationBatch,
        _cancel: &transync::CancellationToken,
    ) -> Result<TranslationBatchResult, TranslatorError> {
        self.batch_calls.fetch_add(1, Ordering::SeqCst);
        self.prompts.lock().unwrap().push((
            batch.units.iter().map(|u| u.unit_id.0.clone()).collect(),
            batch.profile.prompt_body.clone(),
        ));
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
        _cancel: &transync::CancellationToken,
    ) -> Result<Option<Vec<GlossaryEntry>>, TranslatorError> {
        self.extraction_calls.fetch_add(1, Ordering::SeqCst);
        Ok(Some(Vec::new()))
    }
}

fn opts_with(profile: ProfileMetadata) -> TranslateOptions {
    let mut opts = TranslateOptions::default();
    opts.source_language = "en".to_string();
    opts.target_language = "ko".to_string();
    opts.profile = Some(profile);
    opts
}

fn sectioned_entry(target: &str, sections: &[&str]) -> GlossaryEntry {
    GlossaryEntry {
        source_term: "cell".to_string(),
        target_term: target.to_string(),
        note: None,
        scope: GlossaryScope::ConditionalOnSection,
        sections: sections.iter().map(|s| (*s).to_string()).collect(),
    }
}

fn global_entry(target: &str) -> GlossaryEntry {
    GlossaryEntry {
        source_term: "cell".to_string(),
        target_term: target.to_string(),
        note: None,
        scope: GlossaryScope::GlobalAcrossDocument,
        sections: Vec::new(),
    }
}

/// Path 1 — the profile arrives over serde, exactly as a service handing
/// `ProfileMetadata` through a JSON config would build it. It used to be
/// refused here; now it translates, and the term lands only where it says.
#[tokio::test]
async fn a_deserialized_section_scope_translates_and_applies_only_in_its_sections() {
    let profile: ProfileMetadata = serde_json::from_str(
        r#"{
            "slug": "programmatic",
            "version": "1.0.0",
            "prompt_body": "Translate from {{source_language}} to {{target_language}}.",
            "auto_glossary": true,
            "glossary": [
                { "source": "cell", "target": "감방",
                  "scope": "section", "sections": ["Prisons"] }
            ]
        }"#,
    )
    .expect("profile should deserialize");

    let spy = RecordingTranslator::default();
    let out = translate(SOURCE, &opts_with(profile), &spy)
        .await
        .expect("a section-scoped glossary entry no longer fails the run");
    assert!(!out.translated_document.is_empty());
    assert!(spy.batch_calls.load(Ordering::SeqCst) > 0);
    assert_eq!(
        spy.extraction_calls.load(Ordering::SeqCst),
        1,
        "the auto-glossary preflight ran — nothing refuses this profile any more"
    );

    assert!(spy.prompt_for("h2-0006").contains("\"cell\" → \"감방\""));
    for elsewhere in ["p-0001", "h1-0002", "h2-0004"] {
        assert!(
            !spy.prompt_for(elsewhere).contains("감방"),
            "the term must not reach {elsewhere}'s prompt"
        );
    }
}

/// Path 2 — a loader-built profile mutated afterwards, plus the pattern the
/// scope exists for: a global default that a section overrides.
#[tokio::test]
async fn a_section_scoped_entry_overrides_the_global_one_only_in_its_sections() {
    let mut profile = default_profile();
    profile.glossary.push(global_entry("셀"));
    profile.glossary.push(sectioned_entry("감방", &["Prisons"]));

    let spy = RecordingTranslator::default();
    translate(SOURCE, &opts_with(profile), &spy)
        .await
        .expect("both entries load and translate");

    let prisons = spy.prompt_for("h2-0006");
    assert!(prisons.contains("\"cell\" → \"감방\""));
    assert!(
        !prisons.contains("\"cell\" → \"셀\""),
        "the override replaces the default rather than joining it: {prisons}"
    );
    for elsewhere in ["p-0001", "h1-0002", "h2-0004"] {
        let prompt = spy.prompt_for(elsewhere);
        assert!(prompt.contains("\"cell\" → \"셀\""), "{elsewhere}");
        assert!(!prompt.contains("감방"), "{elsewhere}");
    }
}

/// The profile the loader used to refuse loads from TOML, warning-free, with
/// its selectors intact.
#[test]
fn a_section_scoped_profile_loads_from_toml() {
    let toml = r#"
        slug = "scoped"
        version = "1.0.0"
        [system]
        prompt = "Translate."
        [[glossary]]
        source = "cell"
        target = "감방"
        scope = "section"
        sections = ["Prisons"]
    "#;
    let loaded = profile::load_profile(toml).expect("the scope is supported since v0.4.0");
    assert_eq!(loaded.glossary.len(), 1);
    assert!(matches!(
        loaded.glossary[0].scope,
        GlossaryScope::ConditionalOnSection
    ));
    assert_eq!(loaded.glossary[0].sections, vec!["Prisons".to_string()]);
    assert!(
        loaded.load_warnings.is_empty(),
        "{:?}",
        loaded.load_warnings
    );
}

/// Control — a globally-scoped entry is untouched by any of this: it reaches
/// every section's prompt, as it always has.
#[tokio::test]
async fn a_global_entry_still_reaches_every_section() {
    let mut profile = default_profile();
    profile.glossary.push(global_entry("셀"));

    let spy = RecordingTranslator::default();
    let out = translate(SOURCE, &opts_with(profile), &spy)
        .await
        .expect("a global-scope programmatic profile is supported");
    assert!(!out.translated_document.is_empty());
    for id in ["p-0001", "h1-0002", "h2-0004", "h2-0006"] {
        assert!(spy.prompt_for(id).contains("\"cell\" → \"셀\""), "{id}");
    }
}
