//! Gemini `Translator` over either the Gemini Developer API or Vertex AI.
//!
//! The two surfaces share prompts and response parsing, but have distinct
//! authentication and structured-output schema dialects. For Google Cloud's
//! trial credit, select [`GoogleSurface::Vertex`] and ADC; an AI Studio API key
//! pairs with the Developer API and its separate billing account, and a Vertex
//! express-mode key with [`GoogleSurface::VertexExpress`] (DCR-0060).

use std::str::FromStr;
use std::time::Duration;

use google_cloud_auth::credentials::{Builder as AdcBuilder, CacheableResource, Credentials};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use transync::CancellationToken;
use transync::llm::{
    GlossaryEntry, GlossaryExtractionRequest, ProviderFingerprint, TokenizerHint, TranslationBatch,
    TranslationBatchResult, Translator, TranslatorError, prompt,
};
use url::Url;

/// The model's 65,536-token output limit. A caller-owned batching profile
/// supplies its own ceiling; an absent or zero ceiling is omitted on the wire.
pub const FLASH_38_MAX_OUTPUT_TOKENS: u32 = 65_536;
pub const DEFAULT_MODEL: &str = "gemini-3.8-flash";
// Long-context inference may take minutes; the `tokio::select!` around each
// call still drops the HTTP future promptly when the run is cancelled.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(600);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_RESPONSE_BYTES: u64 = 32 * 1024 * 1024;
const MAX_ERROR_RESPONSE_BYTES: u64 = 64 * 1024;
const EXTRACTION_MAX_OUTPUT_TOKENS: u32 = 4_096;

/// A provider model identifier, used verbatim for both requests and cache identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelId(pub String);

impl ModelId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Reject a blank or padded identifier without keeping an in-crate model registry.
    pub fn parse(id: impl Into<String>) -> Result<Self, GoogleConfigError> {
        let id = id.into();
        if id.trim().is_empty() {
            return Err(GoogleConfigError::EmptyModel);
        }
        if id.trim() != id {
            return Err(GoogleConfigError::PaddedModel(id));
        }
        Ok(Self(id))
    }
}

/// Gemini API billing and authentication surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GoogleSurface {
    Developer,
    Vertex {
        project: String,
        location: VertexLocation,
    },
    /// Vertex AI in **express mode**: the same Vertex service and models,
    /// reached with an API key instead of Application Default Credentials.
    ///
    /// It is a separate variant rather than a credential flag on
    /// [`GoogleSurface::Vertex`] because the URL genuinely differs — express
    /// mode carries no project and no location, addressing the publisher
    /// model directly. [`TransyncGoogle::offline`] promises the *same*
    /// fingerprint as a live instance, and the fingerprint follows the
    /// request URL, so the shape has to be derivable from the config alone
    /// rather than from which credential was handed to `try_new`.
    ///
    /// Callers still choose it on the credential axis: the CLI takes
    /// `--google-surface vertex --google-auth api-key`, and an API key present
    /// in the environment never selects it (DCR-0060).
    VertexExpress,
}

/// Supported Vertex AI processing locations for Gemini 3.8 Flash.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VertexLocation {
    Global,
    Us,
    Eu,
}

impl VertexLocation {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Global => "global",
            Self::Us => "us",
            Self::Eu => "eu",
        }
    }

    const fn base_url(self) -> &'static str {
        match self {
            Self::Global => "https://aiplatform.googleapis.com",
            Self::Us => "https://aiplatform.us.rep.googleapis.com",
            Self::Eu => "https://aiplatform.eu.rep.googleapis.com",
        }
    }
}

impl FromStr for VertexLocation {
    type Err = GoogleConfigError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_ascii_lowercase().as_str() {
            "global" => Ok(Self::Global),
            "us" => Ok(Self::Us),
            "eu" => Ok(Self::Eu),
            _ => Err(GoogleConfigError::UnsupportedLocation(value.into())),
        }
    }
}

/// Gemini 3.8 Flash thinking depth. Omission uses the provider default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ThinkingLevel {
    Low,
    Medium,
    High,
}

impl ThinkingLevel {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }
}

impl FromStr for ThinkingLevel {
    type Err = GoogleConfigError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_ascii_lowercase().as_str() {
            "low" => Ok(Self::Low),
            "medium" => Ok(Self::Medium),
            "high" => Ok(Self::High),
            _ => Err(GoogleConfigError::UnsupportedThinkingLevel(value.into())),
        }
    }
}

/// Immutable output-affecting configuration. `base_url` overrides the service
/// origin and may include a gateway prefix or the surface's version segment.
#[derive(Debug, Clone)]
pub struct GoogleConfig {
    pub surface: GoogleSurface,
    pub model: ModelId,
    pub base_url: Option<Url>,
    pub thinking_level: Option<ThinkingLevel>,
}

/// Credential input. Vertex ADC is loaded once at construction and refreshed
/// by Google's authentication library when tokens expire.
#[derive(Debug)]
pub enum GoogleCredentials {
    DeveloperApiKey(SecretString),
    VertexAdc,
    /// A Vertex AI express-mode API key. Pairs with
    /// [`GoogleSurface::VertexExpress`].
    VertexApiKey(SecretString),
}

#[derive(Debug, thiserror::Error)]
pub enum GoogleConfigError {
    #[error("model identifier was empty")]
    EmptyModel,
    #[error("model identifier has surrounding whitespace: {0:?}")]
    PaddedModel(String),
    #[error("Gemini API key was empty")]
    EmptyApiKey,
    #[error("Vertex project was empty")]
    EmptyProject,
    #[error("Vertex project has surrounding whitespace: {0:?}")]
    PaddedProject(String),
    #[error("unsupported Vertex location: {0:?}; expected global, us, or eu")]
    UnsupportedLocation(String),
    #[error("unsupported Gemini thinking level: {0:?}; expected low, medium, or high")]
    UnsupportedThinkingLevel(String),
    #[error("credentials do not match the selected Google API surface")]
    CredentialSurfaceMismatch,
    #[error("invalid Google API base URL: {0}")]
    MalformedUrl(String),
    #[error("could not load Google Application Default Credentials: {0}")]
    Adc(String),
    #[error("could not build HTTP client: {0}")]
    HttpClient(String),
}

enum Auth {
    Developer(SecretString),
    Vertex(Credentials),
    VertexApiKey(SecretString),
    Offline,
}

/// Gemini-backed translator with stable cache identity and pooled HTTP transport.
pub struct TransyncGoogle {
    config: GoogleConfig,
    auth: Auth,
    http: reqwest::Client,
}

impl TransyncGoogle {
    /// Construct a live adapter. This validates all configuration axes and
    /// loads Vertex ADC, but does not send any inference request.
    pub fn try_new(
        config: GoogleConfig,
        credentials: GoogleCredentials,
    ) -> Result<Self, GoogleConfigError> {
        validate_config(&config)?;
        let auth = match (&config.surface, credentials) {
            (GoogleSurface::Developer, GoogleCredentials::DeveloperApiKey(key)) => {
                if key.expose_secret().trim().is_empty() {
                    return Err(GoogleConfigError::EmptyApiKey);
                }
                Auth::Developer(key)
            }
            (GoogleSurface::Vertex { project, .. }, GoogleCredentials::VertexAdc) => {
                let credentials = AdcBuilder::default()
                    .with_scopes(["https://www.googleapis.com/auth/cloud-platform"])
                    // The configured Vertex project is also the default quota
                    // project for this request. The SDK honors an explicit
                    // GOOGLE_CLOUD_QUOTA_PROJECT environment override.
                    .with_quota_project_id(project)
                    .build()
                    .map_err(|e| GoogleConfigError::Adc(e.to_string()))?;
                Auth::Vertex(credentials)
            }
            (GoogleSurface::VertexExpress, GoogleCredentials::VertexApiKey(key)) => {
                if key.expose_secret().trim().is_empty() {
                    return Err(GoogleConfigError::EmptyApiKey);
                }
                Auth::VertexApiKey(key)
            }
            _ => return Err(GoogleConfigError::CredentialSurfaceMismatch),
        };
        Ok(Self {
            config,
            auth,
            http: build_http_client()?,
        })
    }

    /// Validate the same configuration and produce the same fingerprint as a
    /// live instance, but never load credentials or make provider calls.
    pub fn offline(config: GoogleConfig) -> Result<Self, GoogleConfigError> {
        validate_config(&config)?;
        Ok(Self {
            config,
            auth: Auth::Offline,
            http: build_http_client()?,
        })
    }

    pub fn fingerprint(&self) -> ProviderFingerprint {
        let (surface, project, location) = match &self.config.surface {
            GoogleSurface::Developer => ("developer", "", ""),
            GoogleSurface::VertexExpress => ("vertex-express", "", ""),
            GoogleSurface::Vertex { project, location } => {
                ("vertex", project.as_str(), location.as_str())
            }
        };
        // Cache identity follows the effective request URL, so equivalent
        // base spellings (a version suffix or trailing slash) converge.
        let request_url = endpoint(&self.config).expect("validated Google URL");
        let thinking = self
            .config
            .thinking_level
            .map(ThinkingLevel::as_str)
            .unwrap_or("default");
        ProviderFingerprint::new(
            "google",
            &[
                surface,
                project,
                location,
                &self.config.model.0,
                request_url.as_str(),
                thinking,
            ],
        )
    }

    pub fn tokenizer_hint(&self) -> Option<TokenizerHint> {
        Some(TokenizerHint::Cl100kBase)
    }

    async fn generate(&self, body: Value, ceiling: Option<u32>) -> Result<String, TranslatorError> {
        let endpoint = endpoint(&self.config).map_err(|e| TranslatorError::Other(e.to_string()))?;
        let mut request = self.http.post(endpoint).json(&body);
        request = match &self.auth {
            Auth::Developer(key) | Auth::VertexApiKey(key) => {
                // The key rides a header, never the `?key=` query parameter
                // Google's own curl examples use: a URL carries into logs,
                // proxies and error messages, which DCR-0030 forbids.
                request.header("x-goog-api-key", key.expose_secret())
            }
            Auth::Vertex(credentials) => {
                // Use the SDK's complete header set: user ADC can require
                // x-goog-user-project in addition to Authorization.
                let headers = credentials
                    .headers(Default::default())
                    .await
                    .map_err(classify_vertex_credentials_error)?;
                let CacheableResource::New { data, .. } = headers else {
                    return Err(TranslatorError::Authentication(
                        "Vertex ADC supplied no authentication headers".into(),
                    ));
                };
                request.headers(data)
            }
            Auth::Offline => {
                return Err(TranslatorError::NoProviderAvailable(
                    "Google adapter is offline".into(),
                ));
            }
        };
        let response = request
            .send()
            .await
            .map_err(|e| TranslatorError::Network(e.to_string()))?;
        let bytes = read_response(response).await?;
        parse_generation_response(&bytes, &self.config.model, ceiling)
    }
}

fn classify_vertex_credentials_error(
    error: google_cloud_auth::errors::CredentialsError,
) -> TranslatorError {
    let message = format!("could not obtain Vertex ADC headers: {error}");
    if error.is_transient() {
        TranslatorError::Network(message)
    } else {
        TranslatorError::Authentication(message)
    }
}

#[async_trait::async_trait]
impl Translator for TransyncGoogle {
    async fn translate_batch(
        &self,
        batch: TranslationBatch,
        cancel: &CancellationToken,
    ) -> Result<TranslationBatchResult, TranslatorError> {
        tokio::select! {
            biased;
            _ = cancel.cancelled() => Err(TranslatorError::Cancelled),
            result = async {
                let ceiling = batch.profile.batching.target_output_tokens.filter(|n| *n > 0);
                validate_output_ceiling(&self.config.model, ceiling)?;
                let user = prompt::build_user_prompt(&batch)?;
                let schema = prompt::schema_object_for(Some(batch.units.len()));
                let body = generation_body(&self.config, &batch.profile.prompt_body, &user, schema, ceiling);
                let text = self.generate(body, ceiling).await?;
                prompt::parse_batch_output(&text, &batch.batch_id)
            } => result,
        }
    }

    fn fingerprint(&self) -> ProviderFingerprint {
        TransyncGoogle::fingerprint(self)
    }

    fn tokenizer_hint(&self) -> Option<TokenizerHint> {
        TransyncGoogle::tokenizer_hint(self)
    }

    async fn extract_glossary(
        &self,
        req: &GlossaryExtractionRequest,
        cancel: &CancellationToken,
    ) -> Result<Option<Vec<GlossaryEntry>>, TranslatorError> {
        let mut terms = tokio::select! {
            biased;
            _ = cancel.cancelled() => return Err(TranslatorError::Cancelled),
            result = async {
                let user = prompt::build_extraction_user_prompt(req)?;
                let schema = prompt::extraction_schema_object(req.max_terms);
                let body = generation_body(&self.config, prompt::EXTRACTION_SYSTEM_PROMPT, &user,
                    schema, Some(EXTRACTION_MAX_OUTPUT_TOKENS));
                let text = self.generate(body, Some(EXTRACTION_MAX_OUTPUT_TOKENS)).await?;
                prompt::parse_extraction_output(&text)
            } => result?,
        };
        terms.truncate(usize::try_from(req.max_terms).unwrap_or(usize::MAX));
        Ok(Some(terms))
    }
}

fn validate_config(config: &GoogleConfig) -> Result<(), GoogleConfigError> {
    ModelId::parse(config.model.0.clone())?;
    if let GoogleSurface::Vertex { project, .. } = &config.surface {
        if project.trim().is_empty() {
            return Err(GoogleConfigError::EmptyProject);
        }
        if project.trim() != project {
            return Err(GoogleConfigError::PaddedProject(project.clone()));
        }
    }
    if let Some(url) = &config.base_url {
        // Bound for the refusal alone: a base URL with no host names no
        // endpoint. The loopback question below reads the parsed host instead,
        // because the *string* form of an IPv6 host is bracketed.
        url.host_str()
            .ok_or_else(|| GoogleConfigError::MalformedUrl("missing host".into()))?;
        if url.username() != ""
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err(GoogleConfigError::MalformedUrl(
                "base URL cannot contain userinfo, query, or fragment".into(),
            ));
        }
        if url.scheme() != "https" && !(url.scheme() == "http" && is_loopback_host(url)) {
            return Err(GoogleConfigError::MalformedUrl(
                "expected HTTPS, or HTTP on loopback".into(),
            ));
        }
    }
    Ok(())
}

/// Whether a base URL addresses this machine, so cleartext `http` to it
/// never puts the credential or the document on a wire.
///
/// **Matched on the parsed host, never on its string form.** `Url::host_str`
/// serializes an IPv6 host *with its brackets*, so `host_str() == "::1"` is a
/// comparison that can never be true and `http://[::1]:8080/` was refused as
/// remote — a gateway on this very machine, turned away. The IPv4 arm had the
/// narrower version of the same defect: `127.0.0.1` is one address out of the
/// whole `127.0.0.0/8` loopback block, and `127.0.0.2` is as local as `.1` is.
/// Asking the address type closes both, and nothing about this rule is
/// Gemini-specific.
///
/// This is a deliberate twin of `transync_openai`'s
/// `is_cleartext_to_remote_host` / `is_loopback_name` pair (R0002-0005): the
/// two provider crates share no dependency that could hold it, and the rule
/// belongs to neither. Change both together.
fn is_loopback_host(url: &Url) -> bool {
    match url.host() {
        Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
        Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
        Some(url::Host::Domain(name)) => is_loopback_name(name),
        None => false,
    }
}

/// RFC 6761 §6.3: `localhost` and every name under it are reserved for the
/// loopback interface, so a request to one never leaves the machine. The
/// optional fully-qualifying trailing dot names the same host.
fn is_loopback_name(name: &str) -> bool {
    let name = name.strip_suffix('.').unwrap_or(name);
    name.eq_ignore_ascii_case("localhost")
        || name
            .rsplit_once('.')
            .is_some_and(|(_, last)| last.eq_ignore_ascii_case("localhost"))
}

fn build_http_client() -> Result<reqwest::Client, GoogleConfigError> {
    reqwest::Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(REQUEST_TIMEOUT)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| GoogleConfigError::HttpClient(e.to_string()))
}

fn endpoint(config: &GoogleConfig) -> Result<Url, GoogleConfigError> {
    let default = match &config.surface {
        GoogleSurface::Developer => "https://generativelanguage.googleapis.com",
        GoogleSurface::Vertex { location, .. } => location.base_url(),
        GoogleSurface::VertexExpress => "https://aiplatform.googleapis.com",
    };
    let mut url = config
        .base_url
        .clone()
        .unwrap_or_else(|| Url::parse(default).expect("built-in Google origin"));
    let version = match &config.surface {
        GoogleSurface::Developer => "v1beta",
        GoogleSurface::Vertex { .. } | GoogleSurface::VertexExpress => "v1",
    };
    let has_version = url
        .path_segments()
        .and_then(|mut segments| segments.rfind(|s| !s.is_empty()))
        == Some(version);
    let mut segments = url.path_segments_mut().map_err(|_| {
        GoogleConfigError::MalformedUrl("base URL cannot hold path segments".into())
    })?;
    segments.pop_if_empty();
    if !has_version {
        segments.push(version);
    }
    match &config.surface {
        GoogleSurface::Developer => {
            segments.push("models");
        }
        GoogleSurface::VertexExpress => {
            // Express mode addresses the publisher model directly: no
            // project, no location.
            segments.push("publishers").push("google").push("models");
        }
        GoogleSurface::Vertex { project, location } => {
            segments
                .push("projects")
                .push(project)
                .push("locations")
                .push(location.as_str())
                .push("publishers")
                .push("google")
                .push("models");
        }
    }
    segments.push(&format!("{}:generateContent", config.model.0));
    drop(segments);
    Ok(url)
}

/// Refuse a `[batching].target_output_tokens` the default model cannot honour,
/// before a request is built.
///
/// **The variant is the point, because the CLI turns it into an exit code.**
/// This used to raise `Unsupported`, which lands on exit 5 — and `error.rs`
/// says exactly why it lands there: `Unsupported` is "genuinely ambiguous",
/// a code that admits it cannot tell a configuration fault from a document
/// one. That reasoning is right about the variant and wrong about this call
/// site. Nothing here has read the document; this is a pre-flight comparison
/// of one profile number against one published model limit, so the fault is
/// the operator's configuration and nothing else, and the operator's own knob
/// is the whole remediation. `ProviderRejected` with no status carries that:
/// the request was refused rather than failed, `status: None` because no
/// provider was asked, and the CLI maps it to `ConfigurationRejected`. Saying
/// "I cannot tell" about a question with one possible answer is the
/// actively-harmful classification DCR-0023 exists to remove.
///
/// Only the default model is checked. Another model's ceiling is not ours to
/// know, so it rides to the provider, which does know.
fn validate_output_ceiling(model: &ModelId, ceiling: Option<u32>) -> Result<(), TranslatorError> {
    if model.0 == DEFAULT_MODEL
        && let Some(n) = ceiling.filter(|n| *n > FLASH_38_MAX_OUTPUT_TOKENS)
    {
        return Err(TranslatorError::ProviderRejected {
            status: None,
            message: format!(
                "[batching].target_output_tokens is {n}, and {DEFAULT_MODEL} accepts at most \
                 {FLASH_38_MAX_OUTPUT_TOKENS} output tokens including thinking; refused before \
                 sending"
            ),
        });
    }
    Ok(())
}

/// What an operator can actually change when Gemini stops at `MAX_TOKENS`.
///
/// Naming `[batching].target_output_tokens` unconditionally is the obvious
/// phrasing and the wrong one. [`validate_output_ceiling`] refuses a ceiling
/// above the model's published maximum, so at that maximum "raise it" points
/// at a knob this very adapter will not let the operator turn — a remediation
/// that sends them in a circle, which is worse than naming none. Offer the
/// raise only while there is headroom to raise into; otherwise name the knobs
/// that still move. Batch size decides how much output one request asks for,
/// and thinking tokens come out of this same budget, so a lower thinking level
/// buys answer room without touching the ceiling.
fn max_tokens_remediation(model: &ModelId, ceiling: Option<u32>) -> String {
    let at_the_cap =
        model.0 == DEFAULT_MODEL && ceiling.is_none_or(|n| n >= FLASH_38_MAX_OUTPUT_TOKENS);
    if at_the_cap {
        format!(
            "Gemini stopped at MAX_TOKENS with the output ceiling already at {DEFAULT_MODEL}'s \
             maximum of {FLASH_38_MAX_OUTPUT_TOKENS}, which this adapter will not let you raise. \
             Ask for less output per request instead: lower [batching].max_units_per_batch or \
             the input budget, or lower the thinking level — thinking tokens come out of this \
             same ceiling"
        )
    } else {
        let detail = ceiling.map_or_else(
            || "provider output limit".to_string(),
            |n| format!("configured maxOutputTokens={n}"),
        );
        format!(
            "Gemini stopped at MAX_TOKENS ({detail}); raise [batching].target_output_tokens, or \
             ask for less output per request. Thinking tokens also consume the limit"
        )
    }
}

/// Transform the shared JSON Schema to Vertex's OpenAPI-like responseSchema
/// dialect. The Developer surface accepts the shared schema as-is.
fn vertex_schema(mut value: Value) -> Value {
    match &mut value {
        Value::Object(map) => {
            map.remove("additionalProperties");
            if let Some(Value::Array(types)) = map.get("type")
                && types.len() == 2
                && types.iter().any(|v| v == "null")
                && let Some(non_null) = types.iter().find(|v| *v != "null").cloned()
            {
                map.insert(
                    "type".into(),
                    Value::String(non_null.as_str().unwrap_or("string").to_ascii_uppercase()),
                );
                map.insert("nullable".into(), Value::Bool(true));
            } else if let Some(Value::String(ty)) = map.get_mut("type") {
                *ty = ty.to_ascii_uppercase();
            }
            for bound in ["minItems", "maxItems"] {
                if let Some(Value::Number(number)) = map.get(bound) {
                    map.insert(bound.into(), Value::String(number.to_string()));
                }
            }
            if map.get("type") == Some(&Value::String("OBJECT".into()))
                && let Some(required) = map.get("required").cloned()
            {
                map.insert("propertyOrdering".into(), required);
            }
            for child in map.values_mut() {
                let taken = std::mem::take(child);
                *child = vertex_schema(taken);
            }
        }
        Value::Array(items) => {
            for child in items {
                let taken = std::mem::take(child);
                *child = vertex_schema(taken);
            }
        }
        _ => {}
    }
    value
}

fn generation_body(
    config: &GoogleConfig,
    system: &str,
    user: &str,
    schema: Value,
    ceiling: Option<u32>,
) -> Value {
    let mut generation = json!({ "responseMimeType": "application/json" });
    match &config.surface {
        GoogleSurface::Developer => generation["responseJsonSchema"] = schema,
        GoogleSurface::Vertex { .. } | GoogleSurface::VertexExpress => {
            generation["responseSchema"] = vertex_schema(schema)
        }
    }
    if let Some(n) = ceiling.filter(|n| *n > 0) {
        generation["maxOutputTokens"] = json!(n);
    }
    if let Some(level) = config.thinking_level {
        generation["thinkingConfig"] = json!({ "thinkingLevel": level });
    }
    json!({
        "systemInstruction": { "parts": [{ "text": system }] },
        "contents": [{ "role": "user", "parts": [{ "text": user }] }],
        "generationConfig": generation,
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GenerationResponse {
    #[serde(default)]
    candidates: Vec<Candidate>,
    prompt_feedback: Option<PromptFeedback>,
    usage_metadata: Option<UsageMetadata>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Candidate {
    finish_reason: Option<String>,
    content: Option<Content>,
}

#[derive(Deserialize)]
struct Content {
    #[serde(default)]
    parts: Vec<Part>,
}

#[derive(Deserialize)]
struct Part {
    text: Option<String>,
    #[serde(default)]
    thought: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PromptFeedback {
    block_reason: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UsageMetadata {
    prompt_token_count: Option<u64>,
    candidates_token_count: Option<u64>,
    thoughts_token_count: Option<u64>,
    cached_content_token_count: Option<u64>,
}

fn parse_generation_response(
    bytes: &[u8],
    model: &ModelId,
    ceiling: Option<u32>,
) -> Result<String, TranslatorError> {
    let response: GenerationResponse = serde_json::from_slice(bytes).map_err(|e| {
        TranslatorError::MalformedResponse(format!("invalid Gemini response envelope: {e}"))
    })?;
    if let Some(usage) = response.usage_metadata {
        tracing::debug!(
            prompt_tokens = ?usage.prompt_token_count,
            candidate_tokens = ?usage.candidates_token_count,
            thought_tokens = ?usage.thoughts_token_count,
            cached_input_tokens = ?usage.cached_content_token_count,
            "Gemini usage"
        );
    }
    if let Some(reason) = response
        .prompt_feedback
        .and_then(|feedback| feedback.block_reason)
        && reason != "BLOCK_REASON_UNSPECIFIED"
    {
        return Err(TranslatorError::ContentFiltered(format!(
            "Gemini blocked the prompt: {reason}"
        )));
    }
    let candidate = response.candidates.into_iter().next().ok_or_else(|| {
        TranslatorError::MalformedResponse("Gemini response contained no candidates".into())
    })?;
    match candidate.finish_reason.as_deref() {
        Some("STOP") => {}
        Some("MAX_TOKENS") => {
            return Err(TranslatorError::OutputCeilingExhausted(
                max_tokens_remediation(model, ceiling),
            ));
        }
        Some(
            reason @ ("SAFETY" | "RECITATION" | "BLOCKLIST" | "PROHIBITED_CONTENT" | "SPII"
            | "IMAGE_SAFETY"),
        ) => {
            return Err(TranslatorError::ContentFiltered(format!(
                "Gemini stopped at {reason}"
            )));
        }
        Some("MODEL_REFUSAL") => {
            return Err(TranslatorError::ModelRefused(
                "Gemini model refused the request".into(),
            ));
        }
        Some(reason) => {
            return Err(TranslatorError::Other(format!(
                "Gemini stopped at {reason}"
            )));
        }
        None => {
            return Err(TranslatorError::MalformedResponse(
                "Gemini candidate had no finishReason".into(),
            ));
        }
    }
    let text: String = candidate
        .content
        .into_iter()
        .flat_map(|content| content.parts)
        .filter(|part| !part.thought)
        .filter_map(|part| part.text)
        .collect();
    if text.trim().is_empty() {
        return Err(TranslatorError::MalformedResponse(
            "Gemini STOP candidate had no answer text".into(),
        ));
    }
    Ok(text)
}

#[derive(Deserialize)]
struct GoogleErrorEnvelope {
    error: Option<GoogleErrorBody>,
}

#[derive(Deserialize)]
struct GoogleErrorBody {
    message: Option<String>,
    status: Option<String>,
}

async fn read_response(mut response: reqwest::Response) -> Result<Vec<u8>, TranslatorError> {
    let status = response.status();
    let retry_after = response
        .headers()
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| {
            v.parse::<u64>().ok().map(Duration::from_secs).or_else(|| {
                httpdate::parse_http_date(v)
                    .ok()
                    .and_then(|when| when.duration_since(std::time::SystemTime::now()).ok())
            })
        });
    let cap = if status.is_success() {
        MAX_RESPONSE_BYTES
    } else {
        MAX_ERROR_RESPONSE_BYTES
    };
    if status.is_success() && response.content_length().is_some_and(|n| n > cap) {
        return Err(TranslatorError::ResponseTooLarge(format!(
            "Gemini response exceeded {cap} bytes"
        )));
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|e| {
        if status.is_success() {
            TranslatorError::Network(e.to_string())
        } else {
            classify_http(status.as_u16(), retry_after, &bytes)
        }
    })? {
        let room = cap.saturating_sub(bytes.len() as u64) as usize;
        if chunk.len() > room {
            if status.is_success() {
                return Err(TranslatorError::ResponseTooLarge(format!(
                    "Gemini response exceeded {cap} bytes"
                )));
            }
            bytes.extend_from_slice(&chunk[..room]);
            break;
        }
        bytes.extend_from_slice(&chunk);
    }
    if !status.is_success() {
        return Err(classify_http(status.as_u16(), retry_after, &bytes));
    }
    Ok(bytes)
}

fn classify_http(status: u16, retry_after: Option<Duration>, body: &[u8]) -> TranslatorError {
    let parsed: Option<GoogleErrorBody> = serde_json::from_slice::<GoogleErrorEnvelope>(body)
        .ok()
        .and_then(|envelope| envelope.error);
    let detail = parsed
        .as_ref()
        .and_then(|error| error.message.as_ref())
        .map(|message| truncate(message))
        .unwrap_or_else(|| format!("HTTP {status}"));
    match status {
        _ if parsed.as_ref().and_then(|error| error.status.as_deref())
            == Some("RESOURCE_EXHAUSTED") =>
        {
            TranslatorError::RateLimited { retry_after }
        }
        401 | 403 => TranslatorError::Authentication(format!("Gemini HTTP {status}: {detail}")),
        429 => TranslatorError::RateLimited { retry_after },
        408 | 500..=599 => TranslatorError::Network(format!("Gemini HTTP {status}: {detail}")),
        _ => TranslatorError::ProviderRejected {
            status: Some(status),
            message: format!("Gemini HTTP {status}: {detail}"),
        },
    }
}

fn truncate(message: &str) -> String {
    const MAX: usize = 512;
    if message.len() <= MAX {
        return message.into();
    }
    let mut end = MAX;
    while !message.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &message[..end])
}

#[cfg(test)]
mod tests;
