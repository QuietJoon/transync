//! Opt-in, billable full-pipeline smoke tests against the actual Google APIs.
use secrecy::SecretString;
use transync::{ProfileMetadata, TranslateOptions, translate};
use transync_google::{
    GoogleConfig, GoogleCredentials, GoogleSurface, ModelId, TransyncGoogle, VertexLocation,
};

fn enabled() -> bool {
    std::env::var("TRANSYNC_LIVE_SMOKE").ok().as_deref() == Some("1")
}

fn model() -> ModelId {
    ModelId::parse(
        std::env::var("TRANSYNC_LIVE_SMOKE_GOOGLE_MODEL")
            .ok()
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "gemini-3.8-flash".into()),
    )
    .expect("valid test model")
}

const LIVE_SOURCE: &str = "Hello world.\n";

async fn run(adapter: TransyncGoogle, model: &ModelId) {
    let mut options = TranslateOptions::default();
    options.source_language = "en".into();
    options.target_language = "ko".into();
    options.model_id = model.0.clone();
    let mut profile = ProfileMetadata::default();
    profile.batching.target_output_tokens = Some(4_096);
    options.profile = Some(profile);
    // The smoke is evidence of one successful validation path, not a quality
    // evaluation. Bound its token spend to one provider attempt.
    options.max_per_unit_validation_retries = 0;
    options.max_per_batch_schema_retries = 0;
    options.max_per_batch_provider_retries = 0;
    let output = translate(LIVE_SOURCE, &options, &adapter)
        .await
        .expect("live Gemini translate() round-trip");
    let summary = &output.alignment_map.validation_summary;
    assert_eq!(summary.total_units, 1);
    assert_eq!(
        summary.fallback_source, 0,
        "the translated unit must pass validation"
    );
    assert!(!output.translated_document.is_empty());
    assert_ne!(output.translated_document, LIVE_SOURCE);
}

#[tokio::test]
#[ignore = "billable live Gemini Developer API request"]
async fn developer_live_smoke() {
    if !enabled() {
        return;
    }
    let key = std::env::var("GEMINI_API_KEY").expect("GEMINI_API_KEY is required");
    let model = model();
    let adapter = TransyncGoogle::try_new(
        GoogleConfig {
            surface: GoogleSurface::Developer,
            model: model.clone(),
            base_url: None,
            thinking_level: None,
        },
        GoogleCredentials::DeveloperApiKey(SecretString::new(key.into())),
    )
    .unwrap();
    run(adapter, &model).await;
}

#[tokio::test]
#[ignore = "billable live Vertex AI express-mode request"]
async fn vertex_express_live_smoke() {
    if !enabled() {
        return;
    }
    let key = std::env::var("GOOGLE_API_KEY").expect("GOOGLE_API_KEY is required");
    let model = model();
    let adapter = TransyncGoogle::try_new(
        GoogleConfig {
            surface: GoogleSurface::VertexExpress,
            model: model.clone(),
            base_url: None,
            thinking_level: None,
        },
        GoogleCredentials::VertexApiKey(SecretString::new(key.into())),
    )
    .unwrap();
    run(adapter, &model).await;
}

#[tokio::test]
#[ignore = "billable live Vertex AI request"]
async fn vertex_live_smoke() {
    if !enabled() {
        return;
    }
    let project = std::env::var("GOOGLE_CLOUD_PROJECT").expect("GOOGLE_CLOUD_PROJECT is required");
    let model = model();
    let adapter = TransyncGoogle::try_new(
        GoogleConfig {
            surface: GoogleSurface::Vertex {
                project,
                location: VertexLocation::Global,
            },
            model: model.clone(),
            base_url: None,
            thinking_level: None,
        },
        GoogleCredentials::VertexAdc,
    )
    .unwrap();
    run(adapter, &model).await;
}
