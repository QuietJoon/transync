//! The whole stack, keyless: `transync::translate` driven over
//! `TransyncAnthropic` against a canned Messages envelope served from
//! loopback.
//!
//! This is the test that makes the crate's correctness claim without a
//! credential. `tests/live_smoke.rs` proves the *endpoint* exists and answers;
//! this proves everything between the pipeline and the socket — request
//! shaping, structured-output schema, transport, envelope reading, the shared
//! parser, and every validation layer core runs on the way back — agrees. It
//! runs in the default suite, needs no key, and touches nothing but 127.0.0.1.
//!
//! **The server is a translator, not a fixture.** It reads the request the
//! adapter actually sent, pulls the units out of the shared user prompt, and
//! answers one result per unit. A static canned body would only prove the
//! reader parses a string somebody wrote by hand; answering the *real*
//! request is what makes a drift in prompt assembly, schema profiling, or
//! unit-id handling show up here as a failure instead of as a passing test
//! about a stale constant.
//!
//! TRACE: DCR-0029
//! TRACE: SCN-12
//! TRACE: contracts.md §8

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc;

use secrecy::SecretString;
use transync::{TranslateOptions, translate};
use transync_anthropic::{ANTHROPIC_VERSION, Effort, ModelId, TransyncAnthropic};
use url::Url;

/// Heading + paragraph + a two-item list. The same shape the sibling
/// adapter's live smoke uses, and for the same reason: small enough to reason
/// about, structural enough that every validation layer has something to do.
const SOURCE: &str =
    "# Smoke\n\nOne short paragraph about nothing.\n\n- first item\n- second item\n";

/// What one served request revealed about itself, so the assertions can check
/// the wire rather than the code that wrote it.
struct Observed {
    headers: String,
    body: serde_json::Value,
}

/// Serve `count` requests, translating each one, then hand back what was
/// seen. Runs on its own thread; the returned address is bound before this
/// function returns, so the caller cannot race the listener.
fn serve_translating(count: usize) -> (String, mpsc::Receiver<Observed>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
    let addr = listener.local_addr().expect("bound address").to_string();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for _ in 0..count {
            let Ok((mut stream, _)) = listener.accept() else {
                return;
            };
            let Some(observed) = read_request(&mut stream) else {
                return;
            };
            let reply = translate_request(&observed.body);
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{reply}",
                reply.len()
            );
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.flush();
            if tx.send(observed).is_err() {
                return;
            }
        }
    });
    (addr, rx)
}

/// Read one HTTP request far enough to recover its headers and its whole JSON
/// body. `Content-Length` is honest here — the adapter sends a buffered JSON
/// body — so the body is read to exactly that length.
fn read_request(stream: &mut std::net::TcpStream) -> Option<Observed> {
    let mut raw = Vec::new();
    let mut scratch = [0u8; 8192];
    // Read until the header terminator is in hand.
    let header_end = loop {
        let n = stream.read(&mut scratch).ok()?;
        if n == 0 {
            return None;
        }
        raw.extend_from_slice(&scratch[..n]);
        if let Some(at) = raw.windows(4).position(|w| w == b"\r\n\r\n") {
            break at + 4;
        }
    };
    let headers = String::from_utf8_lossy(&raw[..header_end]).to_string();
    let content_length: usize = headers
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse().ok())?
        })
        .expect("the adapter sends a buffered body with a Content-Length");
    // …then until the whole body is in hand.
    while raw.len() - header_end < content_length {
        let n = stream.read(&mut scratch).ok()?;
        if n == 0 {
            break;
        }
        raw.extend_from_slice(&scratch[..n]);
    }
    let body = serde_json::from_slice(&raw[header_end..header_end + content_length])
        .expect("the request body is JSON");
    Some(Observed { headers, body })
}

/// Answer one request the way a cooperating model would: one result per unit,
/// same `unit_id` byte-for-byte, `preserved` with the source echoed back.
///
/// `preserved` rather than a fabricated translation on purpose. It is a legal
/// `output_kind` (§1) that every validation layer accepts, and it keeps the
/// test about *plumbing* — an invented target string would add a second thing
/// that can fail, for a claim this test is not making.
fn translate_request(body: &serde_json::Value) -> String {
    let user_turn = body["messages"][0]["content"]
        .as_str()
        .expect("the single user turn carries the prompt payload");
    let prompt: serde_json::Value =
        serde_json::from_str(user_turn).expect("the user prompt is the shared JSON payload");
    let units: Vec<serde_json::Value> = prompt["units"]
        .as_array()
        .expect("the prompt carries a units array")
        .iter()
        .map(|unit| {
            serde_json::json!({
                "unit_id": unit["unit_id"],
                "output_kind": "preserved",
                "translated_payload": unit["source_payload"],
                "warnings": [],
            })
        })
        .collect();
    let answer =
        serde_json::json!({ "detected_source_language": "en", "units": units }).to_string();
    // The Messages envelope the reader expects: a stop reason, then the JSON
    // in the first text block — behind a thinking block, so the skip is
    // exercised end to end rather than only in the reader's unit tests.
    serde_json::json!({
        "id": "msg_offline",
        "type": "message",
        "role": "assistant",
        "model": "claude-opus-5",
        "stop_reason": "end_turn",
        "content": [
            { "type": "thinking", "thinking": "", "signature": "sig" },
            { "type": "text", "text": answer }
        ],
    })
    .to_string()
}

fn options() -> TranslateOptions {
    let mut opts = TranslateOptions::default();
    opts.target_language = "ko".to_string();
    opts.model_id = "claude-opus-5".to_string();
    opts
}

/// The claim: a real `translate()` run reaches a socket through this adapter,
/// and comes back with output every validation layer accepted — no key, no
/// network beyond loopback.
#[tokio::test]
async fn a_full_translate_run_goes_out_over_the_adapter_and_comes_back_validated() {
    let (addr, seen) = serve_translating(4);
    let provider = TransyncAnthropic::try_new(
        SecretString::new("sk-ant-offline".to_string().into()),
        ModelId::new("claude-opus-5"),
        Some(Url::parse(&format!("http://{addr}")).expect("loopback base URL")),
    )
    .expect("construction with a non-empty key and a valid base URL");

    let output = translate(SOURCE, &options(), &provider)
        .await
        .expect("the offline round-trip must complete");

    // Every source block is paired, and nothing fell back — a fallback here
    // would mean a validation layer rejected what the adapter carried.
    let summary = &output.alignment_map.validation_summary;
    assert!(summary.total_units > 0, "the fixture must produce units");
    assert_eq!(
        summary.fallback_source, 0,
        "a fallback means a layer rejected the adapter's output: {summary:?}"
    );
    let source_blocks = transync_syntax::parser::parse(SOURCE)
        .expect("fixture parses")
        .blocks
        .len();
    assert_eq!(
        output.alignment_map.blocks.len(),
        source_blocks,
        "the alignment map must carry one source/target anchor pair per source block"
    );
    assert!(
        !output.translated_document.is_empty(),
        "regenerated markdown must not be empty"
    );

    // …and the requests that produced it were this provider's shape. At least
    // one request was made; every one of them is checked.
    let observed: Vec<_> = seen.try_iter().collect();
    assert!(
        !observed.is_empty(),
        "the run must have reached the socket at all"
    );
    for request in &observed {
        let headers = request.headers.to_ascii_lowercase();
        assert!(
            headers.contains("x-api-key: sk-ant-offline"),
            "the credential rides in x-api-key: {headers}"
        );
        assert!(
            headers.contains(&format!(
                "anthropic-version: {}",
                ANTHROPIC_VERSION.to_ascii_lowercase()
            )),
            "every request declares the protocol version: {headers}"
        );
        assert!(
            headers.contains("post /v1/messages "),
            "the one endpoint this crate posts to: {headers}"
        );

        let body = &request.body;
        assert_eq!(body["model"], "claude-opus-5");
        assert!(
            body["max_tokens"].is_number(),
            "max_tokens is required on this API and must always be sent: {body}"
        );
        assert!(
            body["system"].is_string(),
            "the system prompt is a top-level field: {body}"
        );
        assert_eq!(
            body["messages"].as_array().expect("messages array").len(),
            1,
            "one user turn: {body}"
        );
        assert_eq!(body["output_config"]["format"]["type"], "json_schema");
        assert!(
            body.get("thinking").is_none(),
            "no thinking parameter is valid family-wide: {body}"
        );
    }
}

/// The schema that goes out on a live-shaped request is the profiled one:
/// none of the keywords this provider's dialect rejects reaches the wire.
/// Pinned through a real run rather than against the builder, so a future
/// change that bypasses the profile pass on the flow's path is caught even if
/// the builder's own test still passes.
#[tokio::test]
async fn the_schema_that_reaches_the_wire_is_in_the_providers_dialect() {
    let (addr, seen) = serve_translating(4);
    let provider = TransyncAnthropic::try_new(
        SecretString::new("sk-ant-offline".to_string().into()),
        ModelId::new("claude-opus-5"),
        Some(Url::parse(&format!("http://{addr}")).expect("loopback base URL")),
    )
    .expect("construction succeeds")
    .with_effort(Effort::Low);

    translate(SOURCE, &options(), &provider)
        .await
        .expect("the offline round-trip must complete");

    let observed: Vec<_> = seen.try_iter().collect();
    assert!(!observed.is_empty(), "the run must have reached the socket");
    for request in &observed {
        let schema = &request.body["output_config"]["format"]["schema"];
        let mut keys = Vec::new();
        collect_keys(schema, &mut keys);
        for rejected in [
            "minItems",
            "maxItems",
            "uniqueItems",
            "minimum",
            "maximum",
            "minLength",
            "maxLength",
            "pattern",
        ] {
            assert!(
                !keys.iter().any(|k| k == rejected),
                "{rejected} must not reach the wire: {schema}"
            );
        }
        // What the provider *demands* is still there.
        assert_eq!(schema["additionalProperties"], false);
        assert!(schema["required"].is_array());
        // …and the effort this instance was built with rode along.
        assert_eq!(request.body["output_config"]["effort"], "low");
    }
}

fn collect_keys(value: &serde_json::Value, out: &mut Vec<String>) {
    match value {
        serde_json::Value::Object(map) => {
            for (k, v) in map {
                out.push(k.clone());
                collect_keys(v, out);
            }
        }
        serde_json::Value::Array(items) => items.iter().for_each(|v| collect_keys(v, out)),
        _ => {}
    }
}

/// DCR-0024: a run whose token has already fired never issues the request.
/// The listener is bound but nothing is ever served, so if the adapter did
/// dispatch, this test would hang on the read rather than fail fast — the
/// biased race is what keeps it a `Cancelled` in microseconds.
#[tokio::test]
async fn an_already_cancelled_token_stops_the_call_before_the_socket() {
    use transync::llm::{BatchId, TranslationBatch, TranslationUnit, Translator};

    let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
    let addr = listener.local_addr().expect("bound address").to_string();

    let provider = TransyncAnthropic::try_new(
        SecretString::new("sk-ant-offline".to_string().into()),
        ModelId::new("claude-opus-5"),
        Some(Url::parse(&format!("http://{addr}")).expect("loopback base URL")),
    )
    .expect("construction succeeds");

    let cancel = transync::CancellationToken::new();
    cancel.cancel();

    let profile =
        transync::profile::render_prompt_body(&transync::profile::default_profile(), "en", "ko");
    let batch = TranslationBatch {
        batch_id: BatchId::new(1),
        units: vec![
            TranslationUnit::new(
                transync::BlockId("p-0001".to_string()),
                transync::BlockKind::Paragraph,
                transync::InputMode::TextFragment,
                "Hello world.".to_string(),
                42,
            )
            .with_batch_id(BatchId::new(1)),
        ],
        source_language: "en".to_string(),
        target_language: "ko".to_string(),
        glossary: profile.glossary.clone(),
        profile,
    };

    let err = provider
        .translate_batch(batch, &cancel)
        .await
        .expect_err("an already-cancelled run must not translate");
    assert!(
        matches!(err, transync::TranslatorError::Cancelled),
        "got {err:?}"
    );
    assert_eq!(err.stable_code(), "provider_cancelled");

    // The extraction preflight answers the same way, and it matters more
    // there: it is the one provider call a run makes *before* any batch, so
    // it is the first stall a cancelled run would notice.
    let req = transync::llm::GlossaryExtractionRequest {
        source_text: "The agent invokes the tool.".to_string(),
        source_truncated: false,
        source_language: "en".to_string(),
        target_language: "ko".to_string(),
        existing_terms: Vec::new(),
        max_terms: 4,
    };
    let err = provider
        .extract_glossary(&req, &cancel)
        .await
        .expect_err("an already-cancelled run must not extract");
    assert!(
        matches!(err, transync::TranslatorError::Cancelled),
        "got {err:?}"
    );

    // Nothing was ever accepted on that listener.
    listener
        .set_nonblocking(true)
        .expect("the listener can be polled");
    assert!(
        listener.accept().is_err(),
        "a cancelled call must not have opened a connection"
    );
}
