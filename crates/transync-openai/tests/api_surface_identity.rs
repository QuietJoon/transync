//! R0001-0031 — the adapter's HTTP surface is decided once, at
//! construction, and both `fingerprint()` and the wire agree with it
//! forever after.
//!
//! The defect this pins was a *timing* defect, not a routing one: the
//! fingerprint and each request used to consult `TRANSYNC_OPENAI_API`
//! independently, so a host that mutated the variable between building a
//! cache key and issuing the request could name one surface in the
//! provider namespace while calling the other. Asserting that requires
//! actually mutating the variable mid-run and then observing where the
//! bytes go — which is why this test owns a whole test binary and a
//! throwaway loopback origin instead of living next to the unit tests.
//!
//! **Why the `unsafe` environment writes are sound here.**
//! `std::env::set_var` is unsafe under edition 2024 because a concurrent
//! reader is a data race. This binary holds exactly one `#[test]`, so no
//! sibling test runs; `#[tokio::test]`'s current-thread runtime drives
//! everything on the test's own thread; and — the load-bearing part — the
//! capture origin's listener is *bound* before the writes but only
//! *served* after the last of them, so every write happens while this
//! thread is the only one that could touch the environment.
//!
//! The same binary closes the loop the other way (ticket `42c8e6d3`): with
//! the variable *live*, a `with_api` pin must still decide the surface. The
//! two claims share one live environment write and one capture origin, so
//! they belong in one test rather than in a sibling that would break the
//! soundness argument above.
//!
//! The origin answers every request with valid HTTP carrying an invalid
//! payload (`{}`), so each call fails in the envelope parser. That is
//! deliberate: the assertion is about the *path* the request reached, and
//! impersonating OpenAI well enough to satisfy the validators would only
//! add fixture surface that could rot.
//!
//! TRACE: SCN-12

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

use secrecy::SecretString;
use transync::llm::{
    BatchId, GlossaryExtractionRequest, TranslationBatch, TranslationUnit, Translator,
};
use transync::profile::{default_profile, render_prompt_body};
use transync::{BlockId, BlockKind, CancellationToken, InputMode};
// Deliberately the crate-root re-exports, not `transync_openai::client::Api`:
// a caller pinning a surface should not need the module path, so the root
// path is what this test compiles against (ticket `42c8e6d3`).
use transync_openai::{Api, ModelId, TransyncOpenAI};
use url::Url;

/// Claim a loopback port without yet serving it, so the base URL is known
/// while the process is still single-threaded.
fn bind_capture_origin() -> (TcpListener, Url) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("binding a loopback port");
    let addr = listener.local_addr().expect("the bound address");
    let base = Url::parse(&format!("http://{addr}")).expect("loopback base URL");
    (listener, base)
}

/// Start answering on an already-bound listener. Every request's target is
/// appended to the returned log; every response is `200 {}`.
fn serve_capture_origin(listener: TcpListener) -> Arc<Mutex<Vec<String>>> {
    let paths: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&paths);
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(stream) = stream else { return };
            let mut reader = BufReader::new(stream);

            let mut request_line = String::new();
            if reader.read_line(&mut request_line).is_err() {
                continue;
            }
            let target = request_line
                .split_whitespace()
                .nth(1)
                .unwrap_or("<no request line>")
                .to_owned();

            // Drain the head, then exactly the declared body, so the
            // client never sees a peer that hung up mid-upload and
            // reports a transport error instead of the parse error the
            // test expects.
            let mut content_length = 0usize;
            loop {
                let mut header = String::new();
                match reader.read_line(&mut header) {
                    Ok(0) => break,
                    Ok(_) => {}
                    Err(_) => break,
                }
                if header.trim().is_empty() {
                    break;
                }
                if let Some((name, value)) = header.split_once(':')
                    && name.eq_ignore_ascii_case("content-length")
                {
                    content_length = value.trim().parse().unwrap_or(0);
                }
            }
            let mut body = vec![0u8; content_length];
            let _ = reader.read_exact(&mut body);

            sink.lock().expect("capture log").push(target);
            let stream = reader.get_mut();
            let _ = stream.write_all(
                b"HTTP/1.1 200 OK\r\ncontent-type: application/json\r\n\
                  content-length: 2\r\nconnection: close\r\n\r\n{}",
            );
            let _ = stream.flush();
        }
    });
    paths
}

fn fixture_batch() -> TranslationBatch {
    let profile = render_prompt_body(&default_profile(), "en", "ko");
    TranslationBatch {
        batch_id: BatchId::new(1),
        units: vec![
            TranslationUnit::new(
                BlockId("p-0001".to_string()),
                BlockKind::Paragraph,
                InputMode::TextFragment,
                "Hello world.".to_string(),
                42,
            )
            .with_batch_id(BatchId::new(1)),
        ],
        source_language: "en".to_string(),
        target_language: "ko".to_string(),
        glossary: profile.glossary.clone(),
        profile,
    }
}

fn fixture_extraction_request() -> GlossaryExtractionRequest {
    GlossaryExtractionRequest {
        source_text: "The agent invokes the tool.".to_string(),
        source_truncated: false,
        source_language: "en".to_string(),
        target_language: "ko".to_string(),
        existing_terms: Vec::new(),
        max_terms: 4,
    }
}

fn provider_for(model: &str, base: &Url) -> TransyncOpenAI {
    TransyncOpenAI::try_new(
        SecretString::new("sk-test".to_string().into()),
        ModelId::new(model),
        Some(base.clone()),
    )
    .expect("construction with a non-empty key and an http base URL")
}

/// Whether a fingerprint carries `surface` as a whole field. Each part is
/// framed by its byte length (R0002-0007), so this cannot be fooled by a
/// substring of the model name or of the base URL — nor by one that spells
/// the framing out.
fn fingerprint_names(provider: &TransyncOpenAI, surface: &str) -> bool {
    provider
        .fingerprint()
        .as_str()
        .contains(&format!("{}\u{1F}{surface}", surface.len()))
}

#[tokio::test]
async fn api_surface_survives_env_mutation_after_construction() {
    let (listener, base) = bind_capture_origin();

    // Normalize first: an ambient override in the developer's shell must
    // not decide what the heuristic is tested against.
    unsafe { std::env::remove_var("TRANSYNC_OPENAI_API") };

    // `gpt-4o` contains no `chat`, so only the heuristic's default rule
    // puts it on Chat Completions — and neither the model name nor the
    // loopback base URL contains the token the fingerprint check looks for.
    let provider = provider_for("gpt-4o", &base);
    assert_eq!(provider.api(), Api::ChatCompletions);
    assert!(fingerprint_names(&provider, "chat"));
    let before = provider.fingerprint();

    // The mutation the old code could not survive. Last environment write:
    // everything below only reads.
    unsafe { std::env::set_var("TRANSYNC_OPENAI_API", "responses") };

    let paths = serve_capture_origin(listener);

    // 1. The cache namespace does not move under a live instance.
    assert_eq!(
        provider.fingerprint(),
        before,
        "the provider fingerprint must not change when TRANSYNC_OPENAI_API does"
    );
    assert!(fingerprint_names(&provider, "chat"));
    assert_eq!(provider.api(), Api::ChatCompletions);

    // 2. Neither does the wire. Both call paths — translation and the
    //    glossary preflight — must reach the surface the fingerprint names.
    let _ = provider
        .translate_batch(fixture_batch(), &CancellationToken::new())
        .await;
    let _ = provider
        .extract_glossary(&fixture_extraction_request(), &CancellationToken::new())
        .await;

    let observed = paths.lock().expect("capture log").clone();
    assert_eq!(
        observed,
        vec![
            "/v1/chat/completions".to_string(),
            "/v1/chat/completions".to_string()
        ],
        "both call paths must go to the surface resolved at construction"
    );

    // 3. Non-vacuity: the environment write really is live, so steps 1 and
    //    2 passed because the value is stored — not because the override
    //    was never observable in the first place.
    let rebuilt = provider_for("gpt-4o", &base);
    assert_eq!(rebuilt.api(), Api::Responses);
    assert!(fingerprint_names(&rebuilt, "responses"));
    assert_ne!(rebuilt.fingerprint(), before);

    let _ = rebuilt
        .translate_batch(fixture_batch(), &CancellationToken::new())
        .await;
    let observed = paths.lock().expect("capture log").clone();
    assert_eq!(
        observed.last().map(String::as_str),
        Some("/v1/responses"),
        "a provider built after the override must honor it"
    );

    // 4. Ticket `42c8e6d3`: `with_api` outranks the environment, on the
    //    wire and in the namespace alike. The variable still says
    //    `responses` — as step 3 just proved for real — so a per-instance
    //    pin back to Chat Completions is the only thing that can put this
    //    request on `/v1/chat/completions`. This is what lets one process
    //    run two adapters on two surfaces without `set_var` between them.
    let pinned = provider_for("gpt-4o", &base).with_api(Api::ChatCompletions);
    assert_eq!(pinned.api(), Api::ChatCompletions);
    assert!(fingerprint_names(&pinned, "chat"));
    assert_ne!(
        pinned.fingerprint(),
        rebuilt.fingerprint(),
        "the pinned adapter must not share the env-resolved one's cache namespace"
    );

    let _ = pinned
        .translate_batch(fixture_batch(), &CancellationToken::new())
        .await;
    let _ = pinned
        .extract_glossary(&fixture_extraction_request(), &CancellationToken::new())
        .await;
    let observed = paths.lock().expect("capture log").clone();
    assert_eq!(
        &observed[observed.len() - 2..],
        [
            "/v1/chat/completions".to_string(),
            "/v1/chat/completions".to_string()
        ],
        "both call paths of a pinned adapter must ignore TRANSYNC_OPENAI_API"
    );
}
