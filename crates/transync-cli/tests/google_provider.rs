//! Google CLI wiring: stub subprocesses check argument resolution, and a
//! loopback Developer API subprocess checks the live adapter and disk cache.

mod common;

use common::ScratchDir;
use std::process::Command;
#[cfg(feature = "test-stub-provider")]
use std::process::Output;
#[cfg(not(feature = "test-stub-provider"))]
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::mpsc::{self, Sender, TryRecvError},
    thread::{self, JoinHandle},
    time::Duration,
};

#[cfg(not(feature = "test-stub-provider"))]
struct LoopbackGoogle {
    base_url: String,
    stop: Sender<()>,
    worker: Option<JoinHandle<Vec<String>>>,
}

#[cfg(not(feature = "test-stub-provider"))]
impl LoopbackGoogle {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback Gemini server");
        let base_url = format!("http://{}/gateway/v1beta", listener.local_addr().unwrap());
        listener.set_nonblocking(true).unwrap();
        let (stop, shutdown) = mpsc::channel();
        let worker = thread::spawn(move || {
            let mut requests = Vec::new();
            while matches!(shutdown.try_recv(), Err(TryRecvError::Empty)) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        // A socket accepted from a nonblocking listener may
                        // also be nonblocking. The request reader relies on
                        // its read timeout instead of a timing-dependent
                        // WouldBlock from a partially delivered request.
                        stream
                            .set_nonblocking(false)
                            .expect("set Gemini connection blocking");
                        let request = read_request(&mut stream);
                        let response = google_response(&request);
                        let http = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response}",
                            response.len()
                        );
                        stream
                            .write_all(http.as_bytes())
                            .expect("write Gemini reply");
                        requests.push(request);
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(error) => panic!("Gemini server accept failed: {error}"),
                }
            }
            requests
        });
        Self {
            base_url,
            stop,
            worker: Some(worker),
        }
    }

    fn finish(mut self) -> Vec<String> {
        self.stop.send(()).expect("stop Gemini server");
        self.worker.take().unwrap().join().expect("Gemini server")
    }
}

#[cfg(not(feature = "test-stub-provider"))]
impl Drop for LoopbackGoogle {
    fn drop(&mut self) {
        let _ = self.stop.send(());
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[cfg(not(feature = "test-stub-provider"))]
fn read_request(stream: &mut TcpStream) -> String {
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("set Gemini request timeout");
    let mut bytes = Vec::new();
    let mut scratch = [0u8; 4096];
    loop {
        let count = stream.read(&mut scratch).expect("read Gemini request");
        assert!(count > 0, "Gemini request ended before its body");
        bytes.extend_from_slice(&scratch[..count]);
        assert!(
            bytes.len() < 1024 * 1024,
            "Gemini fixture request too large"
        );
        if let Some(split) = bytes.windows(4).position(|part| part == b"\r\n\r\n") {
            let headers = String::from_utf8_lossy(&bytes[..split]).to_ascii_lowercase();
            let content_length = headers
                .lines()
                .find_map(|line| line.strip_prefix("content-length: "))
                .and_then(|length| length.parse::<usize>().ok())
                .expect("Gemini request content-length");
            if bytes.len() >= split + 4 + content_length {
                bytes.truncate(split + 4 + content_length);
                return String::from_utf8(bytes).expect("Gemini request UTF-8");
            }
        }
    }
}

#[cfg(not(feature = "test-stub-provider"))]
fn google_response(request: &str) -> String {
    let (_, body) = request.split_once("\r\n\r\n").expect("Gemini request body");
    let envelope: serde_json::Value = serde_json::from_str(body).expect("Gemini request JSON");
    let user = envelope["contents"][0]["parts"][0]["text"]
        .as_str()
        .expect("Gemini user prompt");
    let prompt: serde_json::Value = serde_json::from_str(user).expect("transync prompt JSON");
    let units = prompt["units"].as_array().expect("transync prompt units");
    assert_eq!(units.len(), 1, "the tiny document should make one unit");
    assert_eq!(units[0]["source_payload"], "Hello world.");
    let answer = serde_json::json!({
        "detected_source_language": null,
        "units": [{
            "unit_id": units[0]["unit_id"],
            "output_kind": "translated",
            "translated_payload": "안녕하세요.",
            "warnings": []
        }]
    });
    serde_json::json!({
        "candidates": [{
            "finishReason": "STOP",
            "content": {"parts": [{"text": answer.to_string()}]}
        }]
    })
    .to_string()
}

#[cfg(feature = "test-stub-provider")]
fn run(workdir: &ScratchDir, flags: &[&str], env: &[(&str, &str)]) -> Output {
    let input = workdir.join("source.md");
    std::fs::write(&input, "# Title\n\nHello.\n").expect("write fixture");
    let mut command = Command::new(env!("CARGO_BIN_EXE_transync"));
    command
        .arg("translate")
        .arg("--input")
        .arg(&input)
        .arg("--output")
        .arg(workdir.join("out.md"))
        .arg("--map")
        .arg(workdir.join("map.json"))
        .arg("--target-language")
        .arg("ko")
        .env("TRANSYNC_STUB_ECHO_PATH", workdir.join("provider.json"));
    for name in [
        "TRANSYNC_PROVIDER",
        "TRANSYNC_GOOGLE_SURFACE",
        "TRANSYNC_GOOGLE_MODEL",
        "TRANSYNC_GOOGLE_VERTEX_BASE_URL",
        "TRANSYNC_GOOGLE_DEVELOPER_BASE_URL",
        "TRANSYNC_GOOGLE_THINKING",
        "GOOGLE_CLOUD_PROJECT",
        "GOOGLE_CLOUD_LOCATION",
        "GEMINI_API_KEY",
        "GOOGLE_API_KEY",
    ] {
        command.env_remove(name);
    }
    for (name, value) in env {
        command.env(name, value);
    }
    command.args(flags).output().expect("CLI should run")
}

#[cfg(feature = "test-stub-provider")]
fn provider_record(workdir: &ScratchDir) -> serde_json::Value {
    serde_json::from_slice(&std::fs::read(workdir.join("provider.json")).expect("provider echo"))
        .expect("provider echo should be JSON")
}

#[cfg(feature = "test-stub-provider")]
#[test]
fn google_defaults_to_vertex_and_the_flash_model() {
    let workdir = ScratchDir::new("google-cli-vertex-defaults");
    let output = run(
        &workdir,
        &["--provider", "google"],
        &[("GOOGLE_CLOUD_PROJECT", "test-project")],
    );
    assert!(
        output.status.success(),
        "Google Vertex stub run failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let record = provider_record(&workdir);
    assert_eq!(record["provider"], "google");
    assert_eq!(record["surface"], "vertex");
    assert_eq!(record["project"], "test-project");
    assert_eq!(record["location"], "global");
    assert_eq!(record["thinking"], "low");
    assert_eq!(record["model"], "gemini-3.8-flash");
}

#[cfg(feature = "test-stub-provider")]
#[test]
fn developer_surface_uses_only_its_own_environment() {
    let workdir = ScratchDir::new("google-cli-developer-env");
    let output = run(
        &workdir,
        &[],
        &[
            ("TRANSYNC_PROVIDER", "google"),
            ("TRANSYNC_GOOGLE_SURFACE", "developer"),
            ("TRANSYNC_GOOGLE_MODEL", "gemini-custom"),
            ("TRANSYNC_GOOGLE_THINKING", "medium"),
            (
                "TRANSYNC_GOOGLE_DEVELOPER_BASE_URL",
                "https://developer.example/v1beta",
            ),
            ("TRANSYNC_GOOGLE_VERTEX_BASE_URL", "not a URL"),
            ("GOOGLE_CLOUD_LOCATION", "not-a-location"),
        ],
    );
    assert!(
        output.status.success(),
        "Google Developer stub run failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let record = provider_record(&workdir);
    assert_eq!(record["surface"], "developer");
    assert!(record["project"].is_null());
    assert!(record["location"].is_null());
    assert_eq!(record["thinking"], "medium");
    assert_eq!(record["model"], "gemini-custom");
    assert_eq!(record["base_url"], "https://developer.example/v1beta");
}

#[cfg(feature = "test-stub-provider")]
#[test]
fn google_flags_override_environment_and_invalid_google_configuration_fails_early() {
    let workdir = ScratchDir::new("google-cli-flags");
    let output = run(
        &workdir,
        &[
            "--provider",
            "google",
            "--google-project",
            "flag-project",
            "--google-location",
            "us",
            "--google-thinking",
            "high",
            "--model",
            "gemini-3.8-flash",
            "--base-url",
            "https://flag.example/v1",
        ],
        &[
            ("GOOGLE_CLOUD_PROJECT", "env-project"),
            ("GOOGLE_CLOUD_LOCATION", "invalid-env-location"),
            ("TRANSYNC_GOOGLE_THINKING", "invalid-env-thinking"),
            ("TRANSYNC_GOOGLE_VERTEX_BASE_URL", "not a URL"),
        ],
    );
    assert!(
        output.status.success(),
        "Google flags should override env: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let record = provider_record(&workdir);
    assert_eq!(record["project"], "flag-project");
    assert_eq!(record["location"], "us");
    assert_eq!(record["thinking"], "high");
    assert_eq!(record["base_url"], "https://flag.example/v1");

    let invalid = ScratchDir::new("google-cli-invalid-project");
    let output = run(&invalid, &["--provider", "google"], &[]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("GOOGLE_CLOUD_PROJECT"),
        "missing Vertex project should be an argument error: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(feature = "test-stub-provider")]
#[test]
fn google_stub_rejects_unsafe_base_urls_without_credentials() {
    for (kind, url) in [
        ("userinfo", "https://user@example.test/v1beta"),
        ("query", "https://example.test/v1beta?key=secret"),
        ("fragment", "https://example.test/v1beta#fragment"),
    ] {
        let workdir = ScratchDir::new(&format!("google-cli-bad-url-{kind}"));
        let output = run(
            &workdir,
            &[
                "--provider",
                "google",
                "--google-surface",
                "developer",
                "--base-url",
                url,
            ],
            &[],
        );
        assert_eq!(output.status.code(), Some(1), "{kind} URL should fail");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("base URL cannot contain"),
            "{kind} URL should fail at provider configuration: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

/// DCR-0060: `--google-auth api-key` selects Vertex express mode, which carries
/// no project and no location. An ambient `GOOGLE_CLOUD_PROJECT` is ignored,
/// not refused, because it is environment rather than a request.
#[cfg(feature = "test-stub-provider")]
#[test]
fn google_auth_api_key_selects_vertex_express_without_project_or_location() {
    let workdir = ScratchDir::new("google-cli-express");
    let output = run(
        &workdir,
        &["--provider", "google", "--google-auth", "api-key"],
        &[("GOOGLE_CLOUD_PROJECT", "ambient-project")],
    );
    assert!(
        output.status.success(),
        "Vertex express stub run failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let record = provider_record(&workdir);
    assert_eq!(record["provider"], "google");
    assert_eq!(record["surface"], "vertex-express");
    assert!(record["project"].is_null());
    assert!(record["location"].is_null());
}

/// An explicit project or location beside `--google-auth api-key` would be
/// dropped silently by express mode's URL, so each is an argument error.
#[cfg(feature = "test-stub-provider")]
#[test]
fn google_auth_api_key_refuses_a_project_or_location() {
    for (kind, flag, value) in [
        ("project", "--google-project", "p"),
        ("location", "--google-location", "us"),
    ] {
        let workdir = ScratchDir::new(&format!("google-cli-express-{kind}"));
        let output = run(
            &workdir,
            &[
                "--provider",
                "google",
                "--google-auth",
                "api-key",
                flag,
                value,
            ],
            &[],
        );
        assert_eq!(
            output.status.code(),
            Some(1),
            "{kind} beside express mode should fail"
        );
        assert!(
            String::from_utf8_lossy(&output.stderr)
                .contains("do not apply to --google-auth api-key"),
            "{kind} beside express mode should be an argument error: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

/// `--google-auth` chooses how Vertex authenticates. The Developer API always
/// uses `GEMINI_API_KEY`, so naming the flag there is an argument error.
#[cfg(feature = "test-stub-provider")]
#[test]
fn google_auth_applies_to_the_vertex_surface_only() {
    let workdir = ScratchDir::new("google-cli-auth-developer");
    let output = run(
        &workdir,
        &[
            "--provider",
            "google",
            "--google-surface",
            "developer",
            "--google-auth",
            "adc",
        ],
        &[],
    );
    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("--google-auth applies to --google-surface vertex only"),
        "--google-auth with the Developer surface should fail: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Like every other Google-only flag (DCR-0058), `--google-auth` without
/// `--provider google` is an argument error rather than silently ignored.
#[cfg(feature = "test-stub-provider")]
#[test]
fn google_auth_requires_the_google_provider() {
    let workdir = ScratchDir::new("google-cli-auth-openai");
    let output = run(
        &workdir,
        &["--provider", "openai", "--google-auth", "api-key"],
        &[],
    );
    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("require --provider google"),
        "--google-auth with OpenAI should fail: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// DCR-0058's first mandated refusal, for the four flags that predate
/// `--google-auth`: a `--google-*` flag with a non-Google provider is an
/// argument error, not a flag that is quietly dropped.
///
/// `google_auth_requires_the_google_provider` below covers the same rule for
/// DCR-0060's flag, and covered it alone — the four flags the rule was
/// *written* for had no test at all, so the guard could have been narrowed to
/// `--google-auth` and stayed green. Each flag is driven separately rather
/// than together, because one `||` arm holding the whole condition up reads
/// identically to five working arms.
#[cfg(feature = "test-stub-provider")]
#[test]
fn every_google_flag_requires_the_google_provider() {
    for (flag, value) in [
        ("--google-surface", "developer"),
        ("--google-project", "some-project"),
        ("--google-location", "us"),
        ("--google-thinking", "high"),
    ] {
        let workdir = ScratchDir::new(&format!(
            "google-cli-openai{}",
            flag.trim_start_matches('-')
        ));
        let output = run(&workdir, &["--provider", "openai", flag, value], &[]);
        assert_eq!(
            output.status.code(),
            Some(1),
            "{flag} with --provider openai should be refused"
        );
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("require --provider google"),
            "{flag} with OpenAI should name the rule: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

/// DCR-0058's second mandated refusal: the Developer API addresses no project
/// and no location, so passing one is refused rather than silently dropped.
///
/// The existing Developer test sets `GOOGLE_CLOUD_LOCATION` in the environment
/// and asserts the run *succeeds* with a null location — which is the opposite
/// rule and the correct one, because ambient environment is not a request.
/// These are the explicitly passed flags, and nothing covered them.
#[cfg(feature = "test-stub-provider")]
#[test]
fn the_developer_surface_refuses_an_explicit_project_or_location() {
    for (flag, value) in [
        ("--google-project", "some-project"),
        ("--google-location", "us"),
    ] {
        let workdir = ScratchDir::new(&format!("google-cli-dev{}", flag.trim_start_matches('-')));
        let output = run(
            &workdir,
            &[
                "--provider",
                "google",
                "--google-surface",
                "developer",
                flag,
                value,
            ],
            &[],
        );
        assert_eq!(
            output.status.code(),
            Some(1),
            "{flag} on the Developer surface should be refused"
        );
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("require --google-surface vertex"),
            "{flag} on Developer should name the rule: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

/// An API key in the environment never selects a surface or an auth mode
/// (DCR-0058, DCR-0060): with `GOOGLE_API_KEY` set and no `--google-auth`, a
/// Google run is still ADC Vertex.
#[cfg(feature = "test-stub-provider")]
#[test]
fn an_ambient_google_api_key_never_selects_express_mode() {
    let workdir = ScratchDir::new("google-cli-ambient-key");
    let output = run(
        &workdir,
        &["--provider", "google"],
        &[
            ("GOOGLE_API_KEY", "ambient-key"),
            ("GOOGLE_CLOUD_PROJECT", "test-project"),
        ],
    );
    assert!(
        output.status.success(),
        "Vertex stub run failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let record = provider_record(&workdir);
    assert_eq!(record["surface"], "vertex");
    assert_eq!(record["project"], "test-project");
}

#[cfg(not(feature = "test-stub-provider"))]
#[test]
fn developer_loopback_warms_disk_cache_and_offline_replays_without_a_key() {
    let workdir = ScratchDir::new("google-cli-live-cache");
    let input = workdir.join("source.md");
    let cache_dir = workdir.join("cache");
    std::fs::write(&input, "Hello world.\n").expect("write source");
    let server = LoopbackGoogle::start();

    let run = |name: &str, offline: bool| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_transync"));
        command
            .arg("translate")
            .arg("--input")
            .arg(&input)
            .arg("--output")
            .arg(workdir.join(format!("{name}.md")))
            .arg("--map")
            .arg(workdir.join(format!("{name}.json")))
            .arg("--cache-dir")
            .arg(&cache_dir)
            .arg("--source-language")
            .arg("en")
            .arg("--target-language")
            .arg("ko")
            .arg("--provider")
            .arg("google")
            .arg("--google-surface")
            .arg("developer")
            .arg("--model")
            .arg("gemini-3.8-flash")
            .arg("--base-url")
            .arg(&server.base_url)
            .env("NO_PROXY", "127.0.0.1,localhost")
            .env("no_proxy", "127.0.0.1,localhost");
        for name in [
            "TRANSYNC_PROVIDER",
            "TRANSYNC_GOOGLE_SURFACE",
            "TRANSYNC_GOOGLE_MODEL",
            "TRANSYNC_GOOGLE_DEVELOPER_BASE_URL",
            "TRANSYNC_GOOGLE_THINKING",
            "GEMINI_API_KEY",
        ] {
            command.env_remove(name);
        }
        if offline {
            command.arg("--offline");
        } else {
            command.env("GEMINI_API_KEY", "fixture-key");
        }
        command.output().expect("Google CLI should run")
    };

    let warm = run("warm", false);
    assert!(
        warm.status.success(),
        "live Google translation failed: {}",
        String::from_utf8_lossy(&warm.stderr)
    );
    assert!(cache_dir.join("transync-cache.jsonl").is_file());
    let warm_output = std::fs::read_to_string(workdir.join("warm.md")).unwrap();
    assert!(warm_output.contains("안녕하세요."));

    let offline = run("offline", true);
    assert!(
        offline.status.success(),
        "offline Google cache replay failed: {}",
        String::from_utf8_lossy(&offline.stderr)
    );
    assert_eq!(
        warm_output,
        std::fs::read_to_string(workdir.join("offline.md")).unwrap()
    );
    assert_eq!(
        std::fs::read(workdir.join("warm.json")).unwrap(),
        std::fs::read(workdir.join("offline.json")).unwrap()
    );

    let requests = server.finish();
    assert_eq!(
        requests.len(),
        1,
        "offline replay must make no HTTP request"
    );
    assert!(
        requests[0]
            .starts_with("POST /gateway/v1beta/models/gemini-3.8-flash:generateContent HTTP/1.1")
    );
    let headers = requests[0].split_once("\r\n\r\n").unwrap().0;
    assert!(
        headers
            .lines()
            .any(|line| line.eq_ignore_ascii_case("x-goog-api-key: fixture-key"))
    );
}

#[cfg(not(feature = "test-stub-provider"))]
#[test]
fn custom_profile_only_overrides_the_google_batch_fields_it_sets() {
    let workdir = ScratchDir::new("google-cli-profile-batching");
    let input = workdir.join("source.md");
    std::fs::write(&input, "Hello world.\n").expect("write source");
    let server = LoopbackGoogle::start();
    let profile_head = "slug = \"gemini-test\"\nversion = \"1.0.0\"\n\
                        [system]\nprompt = \"Translate {{source_language}} to {{target_language}}.\"\n";
    let cases = [
        ("no-batching", "", None, 65_536),
        (
            "partial-batching",
            "[batching]\ntarget_input_tokens_per_batch = 7000\n",
            None,
            65_536,
        ),
        (
            "profile-output",
            "[batching]\ntarget_output_tokens = 4096\n",
            None,
            4096,
        ),
        (
            "flag-output",
            "[batching]\ntarget_output_tokens = 4096\n",
            Some("1024"),
            1024,
        ),
    ];

    for (name, batching, flag, _) in cases {
        let profile = workdir.join(format!("{name}.toml"));
        std::fs::write(&profile, format!("{profile_head}{batching}"))
            .expect("write custom profile");
        let mut command = Command::new(env!("CARGO_BIN_EXE_transync"));
        command
            .arg("translate")
            .arg("--input")
            .arg(&input)
            .arg("--output")
            .arg(workdir.join(format!("{name}.md")))
            .arg("--map")
            .arg(workdir.join(format!("{name}.json")))
            .arg("--profile")
            .arg(&profile)
            .arg("--source-language")
            .arg("en")
            .arg("--target-language")
            .arg("ko")
            .arg("--provider")
            .arg("google")
            .arg("--google-surface")
            .arg("developer")
            .arg("--google-thinking")
            .arg("low")
            .arg("--model")
            .arg("gemini-3.8-flash")
            .arg("--base-url")
            .arg(&server.base_url)
            .env("GEMINI_API_KEY", "fixture-key")
            .env("NO_PROXY", "127.0.0.1,localhost")
            .env("no_proxy", "127.0.0.1,localhost");
        if let Some(flag) = flag {
            command.arg("--target-output-tokens").arg(flag);
        }
        let output = command.output().expect("Google CLI should run");
        assert!(
            output.status.success(),
            "{name} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let requests = server.finish();
    assert_eq!(requests.len(), cases.len());
    for (request, (name, _, _, expected_ceiling)) in requests.iter().zip(cases) {
        let (_, body) = request.split_once("\r\n\r\n").expect("request body");
        let envelope: serde_json::Value = serde_json::from_str(body).expect("request JSON");
        assert_eq!(
            envelope["generationConfig"]["maxOutputTokens"], expected_ceiling,
            "{name} should preserve its own output ceiling or inherit Google's"
        );
    }
}

#[cfg(not(feature = "test-stub-provider"))]
#[test]
fn vertex_offline_cold_miss_needs_no_adc() {
    let workdir = ScratchDir::new("google-cli-vertex-offline");
    let input = workdir.join("source.md");
    std::fs::write(&input, "Hello world.\n").expect("write source");
    let output = Command::new(env!("CARGO_BIN_EXE_transync"))
        .arg("translate")
        .arg("--input")
        .arg(&input)
        .arg("--output")
        .arg(workdir.join("out.md"))
        .arg("--map")
        .arg(workdir.join("map.json"))
        .arg("--cache-dir")
        .arg(workdir.join("cache"))
        .arg("--source-language")
        .arg("en")
        .arg("--target-language")
        .arg("ko")
        .arg("--provider")
        .arg("google")
        .arg("--google-surface")
        .arg("vertex")
        .arg("--google-project")
        .arg("fixture-project")
        .arg("--google-location")
        .arg("global")
        .arg("--google-thinking")
        .arg("low")
        .arg("--model")
        .arg("gemini-3.8-flash")
        .arg("--offline")
        .env_remove("GEMINI_API_KEY")
        .env_remove("GOOGLE_CLOUD_PROJECT")
        .env_remove("TRANSYNC_GOOGLE_VERTEX_BASE_URL")
        .env(
            "GOOGLE_APPLICATION_CREDENTIALS",
            workdir.join("missing-adc.json"),
        )
        .output()
        .expect("offline Vertex CLI should run");
    assert_eq!(output.status.code(), Some(6));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Google adapter is offline"), "{stderr}");
    assert!(
        !stderr.contains("ADC"),
        "offline run must not load ADC: {stderr}"
    );
}
