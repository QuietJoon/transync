use super::*;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc;
use transync::{BatchId, BlockId, BlockKind, InputMode, ProfileMetadata, TranslationUnit};

#[derive(Debug)]
struct FixtureVertexCredentials;

impl google_cloud_auth::credentials::CredentialsProvider for FixtureVertexCredentials {
    async fn headers(
        &self,
        _extensions: http::Extensions,
    ) -> Result<CacheableResource<http::HeaderMap>, google_cloud_auth::errors::CredentialsError>
    {
        let mut data = http::HeaderMap::new();
        data.insert(
            http::header::AUTHORIZATION,
            http::HeaderValue::from_static("Bearer fixture-token"),
        );
        data.insert(
            "x-goog-user-project",
            http::HeaderValue::from_static("trial-project"),
        );
        Ok(CacheableResource::New {
            entity_tag: google_cloud_auth::credentials::EntityTag::new(),
            data,
        })
    }

    async fn universe_domain(&self) -> Option<String> {
        None
    }
}

fn developer(base_url: Option<Url>) -> GoogleConfig {
    GoogleConfig {
        surface: GoogleSurface::Developer,
        model: ModelId::parse(DEFAULT_MODEL).unwrap(),
        base_url,
        thinking_level: Some(ThinkingLevel::Low),
    }
}

fn vertex(location: VertexLocation) -> GoogleConfig {
    GoogleConfig {
        surface: GoogleSurface::Vertex {
            project: "trial-project".into(),
            location,
        },
        model: ModelId::parse(DEFAULT_MODEL).unwrap(),
        base_url: None,
        thinking_level: None,
    }
}

fn fixture_batch() -> TranslationBatch {
    let mut profile = ProfileMetadata::default();
    profile.prompt_body =
        "Translate the supplied text from English to Korean. Return the required JSON object."
            .into();
    let batch_id = BatchId::new(7);
    TranslationBatch {
        batch_id: batch_id.clone(),
        units: vec![
            TranslationUnit::new(
                BlockId("p-0001".into()),
                BlockKind::Paragraph,
                InputMode::TextFragment,
                "Hello world.".into(),
                42,
            )
            .with_batch_id(batch_id),
        ],
        source_language: "en".into(),
        target_language: "ko".into(),
        glossary: Vec::new(),
        profile,
    }
}

fn serve_once(response_status: &str, response_body: String) -> (Url, mpsc::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = Url::parse(&format!("http://{}/", listener.local_addr().unwrap())).unwrap();
    let (sender, receiver) = mpsc::channel();
    let status = response_status.to_string();
    std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut bytes = Vec::new();
        let mut scratch = [0u8; 4096];
        loop {
            let count = stream.read(&mut scratch).unwrap();
            if count == 0 {
                break;
            }
            bytes.extend_from_slice(&scratch[..count]);
            if let Some(split) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
                let headers = String::from_utf8_lossy(&bytes[..split]).to_ascii_lowercase();
                let content_length = headers
                    .lines()
                    .find_map(|line| line.strip_prefix("content-length: "))
                    .and_then(|raw| raw.parse::<usize>().ok())
                    .unwrap_or(0);
                if bytes.len() >= split + 4 + content_length {
                    break;
                }
            }
        }
        sender
            .send(String::from_utf8_lossy(&bytes).into_owned())
            .unwrap();
        let response = format!(
            "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response_body}",
            response_body.len()
        );
        stream.write_all(response.as_bytes()).unwrap();
    });
    (base, receiver)
}

#[test]
fn schema_profiles_keep_required_nullable_and_array_bounds() {
    let shared = prompt::schema_object_for(Some(3));
    let developer_body = generation_body(&developer(None), "system", "user", shared.clone(), None);
    let developer_schema = &developer_body["generationConfig"]["responseJsonSchema"];
    assert_eq!(
        developer_schema["properties"]["detected_source_language"]["type"],
        json!(["string", "null"])
    );
    assert_eq!(developer_schema["properties"]["units"]["minItems"], 3);
    assert_eq!(developer_schema["additionalProperties"], false);
    assert_eq!(
        developer_body["generationConfig"]["thinkingConfig"]["thinkingLevel"],
        "LOW"
    );
    assert!(
        developer_body["generationConfig"]
            .get("maxOutputTokens")
            .is_none()
    );

    let vertex_body = generation_body(
        &vertex(VertexLocation::Global),
        "system",
        "user",
        shared,
        Some(1024),
    );
    let vertex_schema = &vertex_body["generationConfig"]["responseSchema"];
    assert_eq!(vertex_schema["type"], "OBJECT");
    assert_eq!(
        vertex_schema["properties"]["detected_source_language"]["type"],
        "STRING"
    );
    assert_eq!(
        vertex_schema["properties"]["detected_source_language"]["nullable"],
        true
    );
    assert_eq!(vertex_schema["properties"]["units"]["minItems"], "3");
    assert_eq!(vertex_schema["properties"]["units"]["maxItems"], "3");
    assert_eq!(
        vertex_schema["required"],
        json!(["detected_source_language", "units"])
    );
    assert_eq!(
        vertex_schema["propertyOrdering"],
        json!(["detected_source_language", "units"])
    );
    assert!(vertex_schema.get("additionalProperties").is_none());
    assert!(
        vertex_schema["properties"]["units"]["items"]
            .get("additionalProperties")
            .is_none()
    );
    assert_eq!(vertex_body["generationConfig"]["maxOutputTokens"], 1024);

    let glossary = super::vertex_schema(prompt::extraction_schema_object(24));
    assert_eq!(glossary["properties"]["terms"]["maxItems"], "24");
    assert_eq!(
        glossary["properties"]["terms"]["items"]["properties"]["note"]["nullable"],
        true
    );
    assert_eq!(
        glossary["properties"]["terms"]["items"]["required"],
        json!(["source", "target", "note"])
    );
}

#[test]
fn endpoint_uses_correct_surface_and_location() {
    assert_eq!(
        endpoint(&developer(None)).unwrap().as_str(),
        "https://generativelanguage.googleapis.com/v1beta/models/gemini-3.8-flash:generateContent"
    );
    assert_eq!(
        endpoint(&vertex(VertexLocation::Global)).unwrap().as_str(),
        "https://aiplatform.googleapis.com/v1/projects/trial-project/locations/global/publishers/google/models/gemini-3.8-flash:generateContent"
    );
    assert!(
        endpoint(&vertex(VertexLocation::Us))
            .unwrap()
            .as_str()
            .starts_with("https://aiplatform.us.rep.googleapis.com/v1/")
    );
    assert!(
        endpoint(&vertex(VertexLocation::Eu))
            .unwrap()
            .as_str()
            .starts_with("https://aiplatform.eu.rep.googleapis.com/v1/")
    );
    let custom = developer(Some(
        Url::parse("https://proxy.example/prefix/v1beta").unwrap(),
    ));
    assert_eq!(
        endpoint(&custom).unwrap().path(),
        "/prefix/v1beta/models/gemini-3.8-flash:generateContent"
    );
}

#[test]
fn offline_validates_axes_and_preserves_live_fingerprint() {
    let config = developer(None);
    let offline = TransyncGoogle::offline(config.clone()).unwrap();
    let live = TransyncGoogle::try_new(
        config,
        GoogleCredentials::DeveloperApiKey(SecretString::new("test-key".into())),
    )
    .unwrap();
    assert_eq!(offline.fingerprint(), live.fingerprint());
    let with_version = TransyncGoogle::offline(developer(Some(
        Url::parse("https://generativelanguage.googleapis.com/v1beta/").unwrap(),
    )))
    .unwrap();
    assert_eq!(offline.fingerprint(), with_version.fingerprint());
    assert!(matches!(
        TransyncGoogle::offline(developer(Some(Url::parse("http://example.com/").unwrap()))),
        Err(GoogleConfigError::MalformedUrl(_))
    ));
    assert!(matches!(
        TransyncGoogle::offline(GoogleConfig {
            model: ModelId(" ".into()),
            ..developer(None)
        }),
        Err(GoogleConfigError::EmptyModel)
    ));
    assert!(matches!(
        TransyncGoogle::try_new(developer(None), GoogleCredentials::VertexAdc),
        Err(GoogleConfigError::CredentialSurfaceMismatch)
    ));
    assert_ne!(
        offline.fingerprint(),
        TransyncGoogle::offline(vertex(VertexLocation::Global))
            .unwrap()
            .fingerprint()
    );
}

/// The envelope's three signals, in the order the parser consults them: a
/// blocked **prompt** outranks the candidate's `finishReason`, which outranks
/// whatever partial text came with it.
///
/// This test was called `finish_reason_precedes_partial_text_and_policy_block`,
/// which names the opposite of the implemented precedence — `promptFeedback`
/// is read first. Nothing caught it because no case carried both signals, so
/// the name was the only statement of an ordering and it was wrong. The order
/// in the code is the right one: `promptFeedback.blockReason` says the
/// **input** was refused, which is why there is no candidate worth reading,
/// while `finishReason` describes a candidate that was at least attempted.
/// Reporting the attempt would name the wrong cause. The both-present case
/// below is what pins it.
#[test]
fn a_blocked_prompt_outranks_finish_reason_which_outranks_partial_text() {
    let model = ModelId::new(DEFAULT_MODEL);
    let partial = json!({"candidates":[{"finishReason":"MAX_TOKENS","content":{"parts":[{"text":"{\"terms\":[]}"}]}}]});
    assert!(matches!(
        parse_generation_response(partial.to_string().as_bytes(), &model, Some(1024)),
        Err(TranslatorError::OutputCeilingExhausted(_))
    ));
    let blocked = json!({"promptFeedback":{"blockReason":"SAFETY"},"candidates":[]});
    assert!(matches!(
        parse_generation_response(blocked.to_string().as_bytes(), &model, None),
        Err(TranslatorError::ContentFiltered(_))
    ));
    let both = json!({
        "promptFeedback":{"blockReason":"SAFETY"},
        "candidates":[{"finishReason":"MAX_TOKENS","content":{"parts":[{"text":"partial"}]}}]
    });
    assert!(
        matches!(
            parse_generation_response(both.to_string().as_bytes(), &model, Some(1024)),
            Err(TranslatorError::ContentFiltered(_))
        ),
        "a refused prompt is the cause; the candidate's own finishReason is a consequence"
    );
    let unspecified = json!({
        "promptFeedback":{"blockReason":"BLOCK_REASON_UNSPECIFIED"},
        "candidates":[{"finishReason":"STOP","content":{"parts":[{"text":"{\"terms\":[]}"}]}}]
    });
    assert_eq!(
        parse_generation_response(unspecified.to_string().as_bytes(), &model, None).unwrap(),
        "{\"terms\":[]}",
        "BLOCK_REASON_UNSPECIFIED is the absence of a block, not a block"
    );
    let split = json!({"candidates":[{"finishReason":"STOP","content":{"parts":[
        {"text":"private reasoning","thought":true},{"text":"{\"terms\":"},{"text":"[]}"}
    ]}}]});
    assert_eq!(
        parse_generation_response(split.to_string().as_bytes(), &model, None).unwrap(),
        "{\"terms\":[]}"
    );
}

/// A ceiling the default model cannot honour is the **operator's**
/// configuration, so it must not land on the exit code that means "I cannot
/// tell a configuration fault from a document one". `ProviderRejected` with no
/// status is the one that says a request was refused with nothing asked.
#[test]
fn an_impossible_output_ceiling_is_a_configuration_refusal_not_an_ambiguous_one() {
    let err = validate_output_ceiling(&ModelId::new(DEFAULT_MODEL), Some(65_537)).unwrap_err();
    let TranslatorError::ProviderRejected { status, message } = err else {
        panic!("an impossible ceiling must be a request refusal, got {err:?}");
    };
    assert_eq!(status, None, "no provider was asked, so there is no status");
    assert!(
        message.contains("65537") && message.contains("target_output_tokens"),
        "the refusal names the value it rejected and the knob that set it: {message}"
    );
    assert!(validate_output_ceiling(&ModelId::new(DEFAULT_MODEL), Some(65_536)).is_ok());
    assert!(
        validate_output_ceiling(&ModelId::new("gemini-some-future-model"), Some(1_000_000)).is_ok(),
        "another model's ceiling is not ours to know"
    );
}

/// At the model's own maximum, "raise the ceiling" names a knob this adapter
/// refuses to let the operator turn. The message must name one that moves.
#[test]
fn the_max_tokens_remediation_is_never_circular() {
    let model = ModelId::new(DEFAULT_MODEL);
    for ceiling in [None, Some(FLASH_38_MAX_OUTPUT_TOKENS)] {
        let m = max_tokens_remediation(&model, ceiling);
        assert!(
            !m.contains("raise [batching].target_output_tokens"),
            "at the cap the raise is impossible, so it must not be offered: {m}"
        );
        assert!(
            m.contains("max_units_per_batch") && m.contains("thinking"),
            "it must name the knobs that do still move: {m}"
        );
    }
    let headroom = max_tokens_remediation(&model, Some(1_024));
    assert!(
        headroom.contains("raise [batching].target_output_tokens"),
        "below the cap the raise is the obvious fix and is offered: {headroom}"
    );
}

/// `Url::host_str` brackets an IPv6 host, so the old `host == "::1"` could
/// never be true and a gateway on this machine was refused as remote. The
/// IPv4 side had the narrower form of the same defect: `127.0.0.1` is one
/// address in a `/8` that is loopback all the way through.
#[test]
fn cleartext_is_allowed_to_every_loopback_spelling_and_no_other() {
    for allowed in [
        "http://localhost:8080/v1beta",
        "http://LocalHost:8080/v1beta",
        "http://localhost./v1beta",
        "http://gateway.localhost/v1beta",
        "http://127.0.0.1:8080/v1beta",
        "http://127.0.0.2:8080/v1beta",
        "http://[::1]:8080/v1beta",
        "https://generativelanguage.googleapis.com/v1beta",
    ] {
        assert!(
            TransyncGoogle::offline(developer(Some(Url::parse(allowed).unwrap()))).is_ok(),
            "{allowed} should be accepted"
        );
    }
    for refused in [
        "http://example.com/v1beta",
        "http://10.0.0.1/v1beta",
        "http://[2001:db8::1]/v1beta",
        "http://notlocalhost/v1beta",
        "http://localhost.example.com/v1beta",
    ] {
        assert!(
            matches!(
                TransyncGoogle::offline(developer(Some(Url::parse(refused).unwrap()))),
                Err(GoogleConfigError::MalformedUrl(_))
            ),
            "{refused} is not this machine, so cleartext must be refused"
        );
    }
}

#[test]
fn ceiling_and_http_errors_keep_retry_taxonomy() {
    assert!(validate_output_ceiling(&ModelId::new(DEFAULT_MODEL), None).is_ok());
    assert!(matches!(
        classify_http(
            429,
            Some(Duration::from_secs(2)),
            br#"{"error":{"status":"RESOURCE_EXHAUSTED"}}"#
        ),
        TranslatorError::RateLimited {
            retry_after: Some(_)
        }
    ));
    assert!(matches!(
        classify_http(403, None, br#"{"error":{"message":"permission denied"}}"#),
        TranslatorError::Authentication(_)
    ));
    assert!(matches!(
        classify_http(
            403,
            Some(Duration::from_secs(3)),
            br#"{"error":{"status":"RESOURCE_EXHAUSTED","message":"quota exceeded"}}"#
        ),
        TranslatorError::RateLimited {
            retry_after: Some(_)
        }
    ));
    assert!(matches!(
        classify_http(503, None, b""),
        TranslatorError::Network(_)
    ));
}

#[tokio::test]
async fn developer_loopback_uses_key_header_and_shared_parser() {
    let model_answer = json!({
        "detected_source_language": "en",
        "units": [{
            "unit_id": "p-0001", "output_kind": "translated",
            "translated_payload": "안녕, 세상.", "warnings": []
        }]
    });
    let envelope = json!({"candidates":[{"finishReason":"STOP","content":{"parts":[{"text":model_answer.to_string()}]}}],
        "usageMetadata":{"promptTokenCount":12,"candidatesTokenCount":10,"thoughtsTokenCount":2}});
    let (base, received) = serve_once("200 OK", envelope.to_string());
    let adapter = TransyncGoogle::try_new(
        developer(Some(base)),
        GoogleCredentials::DeveloperApiKey(SecretString::new("loopback-key".into())),
    )
    .unwrap();
    let translated = adapter
        .translate_batch(fixture_batch(), &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(translated.units[0].translated_payload, "안녕, 세상.");
    let request = received.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(request.starts_with("POST /v1beta/models/gemini-3.8-flash:generateContent HTTP/1.1"));
    assert!(request.to_ascii_lowercase().contains("x-goog-api-key:"));
    assert!(!request.to_ascii_lowercase().contains("authorization:"));
    let body: Value = serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap();
    assert!(body["generationConfig"].get("responseJsonSchema").is_some());
    assert!(body["generationConfig"].get("responseSchema").is_none());
    assert_eq!(
        body["generationConfig"]["thinkingConfig"]["thinkingLevel"],
        "LOW"
    );
}

#[tokio::test]
async fn vertex_loopback_forwards_adc_auth_and_quota_project_headers() {
    let answer = json!({
        "detected_source_language": null,
        "units": [{
            "unit_id": "p-0001", "output_kind": "translated",
            "translated_payload": "안녕, 세상.", "warnings": []
        }]
    });
    let envelope = json!({"candidates":[{"finishReason":"STOP","content":{"parts":[{"text":answer.to_string()}]}}]});
    let (base, received) = serve_once("200 OK", envelope.to_string());
    let mut config = vertex(VertexLocation::Global);
    config.base_url = Some(base);
    let adapter = TransyncGoogle {
        config,
        auth: Auth::Vertex(Credentials::from(FixtureVertexCredentials)),
        http: build_http_client().unwrap(),
    };

    let translated = adapter
        .translate_batch(fixture_batch(), &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(translated.units[0].translated_payload, "안녕, 세상.");
    let request = received.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(request.starts_with("POST /v1/projects/trial-project/locations/global/publishers/google/models/gemini-3.8-flash:generateContent HTTP/1.1"));
    let headers = request
        .split_once("\r\n\r\n")
        .unwrap()
        .0
        .to_ascii_lowercase();
    assert!(headers.contains("authorization: bearer fixture-token"));
    assert!(headers.contains("x-goog-user-project: trial-project"));
    assert!(!headers.contains("x-goog-api-key:"));
}

#[test]
fn vertex_adc_transient_failures_remain_retryable() {
    let transient = google_cloud_auth::errors::CredentialsError::from_msg(true, "timeout");
    assert!(matches!(
        classify_vertex_credentials_error(transient),
        TranslatorError::Network(_)
    ));
    let permanent = google_cloud_auth::errors::CredentialsError::from_msg(false, "missing ADC");
    assert!(matches!(
        classify_vertex_credentials_error(permanent),
        TranslatorError::Authentication(_)
    ));
}

#[tokio::test]
async fn already_cancelled_offline_request_is_typed_cancelled() {
    let adapter = TransyncGoogle::offline(developer(None)).unwrap();
    let cancel = CancellationToken::new();
    cancel.cancel();
    assert!(matches!(
        adapter.translate_batch(fixture_batch(), &cancel).await,
        Err(TranslatorError::Cancelled)
    ));
    let request = GlossaryExtractionRequest {
        source_text: "The agent invokes the tool.".into(),
        source_truncated: false,
        source_language: "en".into(),
        target_language: "ko".into(),
        existing_terms: Vec::new(),
        max_terms: 1,
    };
    assert!(matches!(
        adapter.extract_glossary(&request, &cancel).await,
        Err(TranslatorError::Cancelled)
    ));
}

/// DCR-0060: Vertex express mode addresses the publisher model directly — no
/// project and no location — and is reached with an API key. Getting this URL
/// wrong is a silent 404 rather than a compile error, so it is pinned here.
#[test]
fn vertex_express_addresses_the_publisher_model_without_project_or_location() {
    let config = GoogleConfig {
        surface: GoogleSurface::VertexExpress,
        model: ModelId::parse("gemini-3.8-flash".to_string()).unwrap(),
        base_url: None,
        thinking_level: None,
    };
    let url = endpoint(&config).expect("express endpoint builds");
    assert_eq!(
        url.as_str(),
        "https://aiplatform.googleapis.com/v1/publishers/google/models/gemini-3.8-flash:generateContent"
    );
    assert!(
        !url.as_str().contains("projects/"),
        "express carries no project"
    );
    assert!(
        !url.as_str().contains("locations/"),
        "express carries no location"
    );
}

/// The credential and the surface are paired: an express surface takes an
/// express key, and every other combination is refused rather than sent.
#[test]
fn vertex_express_pairs_only_with_an_api_key() {
    let config = || GoogleConfig {
        surface: GoogleSurface::VertexExpress,
        model: ModelId::parse("gemini-3.8-flash".to_string()).unwrap(),
        base_url: None,
        thinking_level: None,
    };
    assert!(
        TransyncGoogle::try_new(
            config(),
            GoogleCredentials::VertexApiKey(SecretString::new("k".to_string().into()))
        )
        .is_ok()
    );
    assert!(matches!(
        TransyncGoogle::try_new(config(), GoogleCredentials::VertexAdc),
        Err(GoogleConfigError::CredentialSurfaceMismatch)
    ));
    assert!(matches!(
        TransyncGoogle::try_new(
            config(),
            GoogleCredentials::VertexApiKey(SecretString::new("   ".to_string().into()))
        ),
        Err(GoogleConfigError::EmptyApiKey)
    ));
    // The other direction: a Developer key on the express surface, and an
    // express key on either other surface, are refused too.
    assert!(matches!(
        TransyncGoogle::try_new(
            config(),
            GoogleCredentials::DeveloperApiKey(SecretString::new("k".to_string().into()))
        ),
        Err(GoogleConfigError::CredentialSurfaceMismatch)
    ));
    assert!(matches!(
        TransyncGoogle::try_new(
            developer(None),
            GoogleCredentials::VertexApiKey(SecretString::new("k".to_string().into()))
        ),
        Err(GoogleConfigError::CredentialSurfaceMismatch)
    ));
    assert!(matches!(
        TransyncGoogle::try_new(
            vertex(VertexLocation::Global),
            GoogleCredentials::VertexApiKey(SecretString::new("k".to_string().into()))
        ),
        Err(GoogleConfigError::CredentialSurfaceMismatch)
    ));
}

/// Express and ADC Vertex are the same service but different request URLs, so
/// they must not share a cache entry.
#[test]
fn vertex_express_and_adc_vertex_do_not_share_a_fingerprint() {
    let express = TransyncGoogle::offline(GoogleConfig {
        surface: GoogleSurface::VertexExpress,
        model: ModelId::parse("gemini-3.8-flash".to_string()).unwrap(),
        base_url: None,
        thinking_level: None,
    })
    .unwrap();
    let adc = TransyncGoogle::offline(vertex(VertexLocation::Global)).unwrap();
    assert_ne!(express.fingerprint(), adc.fingerprint());
}
