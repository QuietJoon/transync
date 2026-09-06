//! End-to-end CLI smoke. Drives `transync translate` via `Command` against
//! the SCN-14 fixture, asserts the four output paths exist + are non-empty
//! and that the alignment-map JSON parses to the durable wire shape.
//! Also exercises the documented exit codes (0, 1, 2, 3, 4, 5, 6, 7) per
//! `docs/architecture/contracts.md` §6, plus the EXT-2026-07 P1-6/P1-7
//! `--out-dir` staged directory publish and `--max-input-bytes` admission
//! control.
//!
//! Requires the `test-stub-provider` Cargo feature so no API key is needed.
//! Run: `cargo test -p transync-cli --features test-stub-provider`.
//!
//! IMPORTANT: `cargo test --workspace` does NOT compile this file because
//! it is feature-gated. Both commands must run to exercise the full
//! published surface — `scripts/smoke.sh` does this automatically.
//!
//! TRACE: SCN-12
//! TRACE: SL-12
//! TRACE: contracts.md §6

mod common;

#[cfg(feature = "test-stub-provider")]
use common::ScratchDir;
#[cfg(feature = "test-stub-provider")]
use std::process::Command;

#[cfg(feature = "test-stub-provider")]
fn fixture_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../transync/tests/fixtures/scn-14-full.md")
        .canonicalize()
        .expect("fixture should exist")
}

#[cfg(feature = "test-stub-provider")]
fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_transync")
}

/// A minimal HTML *document* — the input shape ti `13e145` refuses. The blank
/// lines are what makes it more than a single raw-HTML block: each run that
/// does not open with a tag re-enters as Markdown. The indented run is the
/// half that changes the document's shape: it becomes an indented code block,
/// which the engine translates and re-emits FENCED, pinned by
/// `cli_html_input_re_emits_an_indented_run_as_a_fenced_block` below.
#[cfg(feature = "test-stub-provider")]
const HTML_DOCUMENT: &str = "<!DOCTYPE html>\n<html lang=\"en\">\n<body>\n<p>Hello.</p>\n\n    A four-space-indented line.\n\n</body>\n</html>\n";

#[cfg(feature = "test-stub-provider")]
#[test]
fn cli_translate_smoke() {
    let workdir = ScratchDir::new("transync-cli-smoke");
    let input = fixture_path();
    let out_md = workdir.join("out.md");
    let out_json = workdir.join("out.json");
    let html_out = workdir.join("html");

    let status = Command::new(bin())
        .arg("translate")
        .arg("--input")
        .arg(&input)
        .arg("--output")
        .arg(&out_md)
        .arg("--map")
        .arg(&out_json)
        .arg("--html-out")
        .arg(&html_out)
        .arg("--target-language")
        .arg("ko")
        .status()
        .expect("transync binary must run");

    assert!(
        status.success(),
        "transync translate exit code: {:?}",
        status.code()
    );

    for path in [
        &out_md,
        &out_json,
        &html_out.join("index.html"),
        &html_out.join("source.html"),
        &html_out.join("target.html"),
        &html_out.join("alignment.json"),
        &html_out.join("sync.js"),
        &html_out.join("purify.min.js"),
    ] {
        let meta = std::fs::metadata(path)
            .unwrap_or_else(|e| panic!("expected {} to exist: {}", path.display(), e));
        assert!(
            meta.len() > 0,
            "expected {} to be non-empty",
            path.display()
        );
    }

    // `R0001-0084` in the removed `reviews/reviewed/0001.md`: don't just
    // check non-empty; parse the alignment JSON
    // and verify the durable wire shape, and check that the rendered
    // panes carry data-sync-id attributes that the JS engine can find.
    let alignment_text =
        std::fs::read_to_string(&out_json).expect("alignment JSON should be readable");
    let alignment: serde_json::Value =
        serde_json::from_str(&alignment_text).expect("alignment JSON should parse");
    assert_eq!(
        alignment["schema_version"].as_str(),
        Some("1.3.0"),
        "alignment_map.schema_version must be 1.3.0"
    );
    assert!(
        alignment["blocks"]
            .as_array()
            .is_some_and(|a| !a.is_empty()),
        "alignment_map.blocks must be a non-empty array"
    );
    assert!(
        alignment["generator"]["name"].as_str() == Some("transync"),
        "alignment_map.generator.name must be 'transync'"
    );

    // ti 490d97 wave 6 (spec §11's cheap pin, written to §3's normative
    // definition): the map declares the run's intake, and every row declares
    // its block's SPELLING — which over the SCN-14 corpus (it contains raw
    // html islands) makes the html arm non-vacuous.
    assert_eq!(
        alignment["input_format"].as_str(),
        Some("markdown"),
        "a Markdown run's map declares input_format markdown"
    );
    for row in alignment["blocks"].as_array().expect("blocks array") {
        let expected = if row["block_kind"].as_str() == Some("html") {
            "html"
        } else {
            "markdown"
        };
        assert_eq!(
            row["source_format"].as_str(),
            Some(expected),
            "row {}: source_format is the block's spelling",
            row["source_block_id"]
        );
    }

    let source_html = std::fs::read_to_string(html_out.join("source.html"))
        .expect("source.html should be readable");
    let target_html = std::fs::read_to_string(html_out.join("target.html"))
        .expect("target.html should be readable");
    assert!(
        source_html.contains("data-sync-id="),
        "source.html must carry data-sync-id attributes for the JS engine"
    );
    assert!(
        target_html.contains("data-sync-id="),
        "target.html must carry data-sync-id attributes for the JS engine"
    );
}

/// Exit code 1 — argument error. Missing required `--input`.
///
/// TRACE: SCN-12
#[cfg(feature = "test-stub-provider")]
#[test]
fn cli_exit_1_argument_error() {
    let status = Command::new(bin())
        .arg("translate")
        .arg("--target-language")
        .arg("ko")
        .status()
        .expect("transync binary must run");
    assert_eq!(
        status.code(),
        Some(1),
        "missing required arg should exit 1, got {:?}",
        status.code()
    );
}

/// ti `30a744`: `--offline` without `--cache-dir` is refused at argument time.
///
/// Not a stylistic guard. Without a cache that outlives the process the run
/// gets a fresh in-memory one, so the FIRST unit misses by construction and an
/// offline run that misses cannot proceed — the flag would have exactly one
/// reachable outcome. Refusing here spends no parse and the message names the
/// flag to add, instead of arriving at the same failure after packing batches.
///
/// Gated with the rest of this file's subprocess tests: the harness that
/// locates the binary and the fixture is stub-only, and the guard it exercises
/// runs in both builds.
#[cfg(feature = "test-stub-provider")]
#[test]
fn cli_offline_without_cache_dir_is_an_argument_error() {
    let workdir = ScratchDir::new("transync-cli-offline");
    let input = fixture_path();
    // An output target is supplied so this reaches the `--offline` guard: the
    // output-target requirement is checked first, and the first draft of this
    // test tripped on that instead — which is the test discriminating, and the
    // reason it asserts the MESSAGE rather than only the code.
    let out = Command::new(bin())
        .arg("translate")
        .arg("--input")
        .arg(&input)
        .arg("--target-language")
        .arg("ko")
        .arg("--out-dir")
        .arg(workdir.path().join("out"))
        .arg("--offline")
        .output()
        .expect("transync binary must run");
    assert_eq!(
        out.status.code(),
        Some(1),
        "--offline with no --cache-dir should exit 1, got {:?}",
        out.status.code()
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("--cache-dir"),
        "the refusal must name the flag to add; got: {stderr}"
    );
}

/// Exit code 2 — input read failure. Non-existent input file.
///
/// TRACE: SCN-12
#[cfg(feature = "test-stub-provider")]
#[test]
fn cli_exit_2_input_read_failure() {
    let workdir = ScratchDir::new("transync-cli-exit2");
    let status = Command::new(bin())
        .arg("translate")
        .arg("--input")
        .arg(workdir.join("does-not-exist.md"))
        .arg("--output")
        .arg(workdir.join("out.md"))
        .arg("--map")
        .arg(workdir.join("out.json"))
        .arg("--target-language")
        .arg("ko")
        .status()
        .expect("transync binary must run");
    assert_eq!(
        status.code(),
        Some(2),
        "missing input should exit 2, got {:?}",
        status.code()
    );
}

/// Exit code 3 — every translatable unit fell back to source. Triggered
/// by `TRANSYNC_STUB_MODE=fail`, which swaps the EchoTranslator for the
/// AlwaysFailsTranslator so every unit returns FailedNeedsFallback.
///
/// TRACE: SCN-08
/// TRACE: contracts.md §6
#[cfg(feature = "test-stub-provider")]
#[test]
fn cli_exit_3_all_units_fell_back() {
    let workdir = ScratchDir::new("transync-cli-exit3");
    let status = Command::new(bin())
        .env("TRANSYNC_STUB_MODE", "fail")
        .arg("translate")
        .arg("--input")
        .arg(fixture_path())
        .arg("--output")
        .arg(workdir.join("out.md"))
        .arg("--map")
        .arg(workdir.join("out.json"))
        .arg("--target-language")
        .arg("ko")
        .status()
        .expect("transync binary must run");
    assert_eq!(
        status.code(),
        Some(3),
        "all-fallback should exit 3, got {:?}",
        status.code()
    );
}

/// Exit code 4 — write failure. Pass a `--output` path whose parent path
/// is a regular file, so `create_dir_all` returns `ENOTDIR`.
///
/// TRACE: SCN-12
#[cfg(feature = "test-stub-provider")]
#[test]
fn cli_exit_4_write_failure() {
    let workdir = ScratchDir::new("transync-cli-exit4");
    let blocker = workdir.join("blocker");
    std::fs::write(&blocker, b"not a directory").unwrap();
    let bad_output = blocker.join("subdir").join("out.md");

    let status = Command::new(bin())
        .arg("translate")
        .arg("--input")
        .arg(fixture_path())
        .arg("--output")
        .arg(&bad_output)
        .arg("--map")
        .arg(workdir.join("out.json"))
        .arg("--target-language")
        .arg("ko")
        .status()
        .expect("transync binary must run");
    assert_eq!(
        status.code(),
        Some(4),
        "unwritable output parent should exit 4, got {:?}",
        status.code()
    );
}

/// Drive one `TRANSYNC_STUB_MODE` through a full `translate` run and return
/// the process exit code alongside the scratch directory it wrote into.
///
/// Each mode makes the stub raise one terminal `TranslatorError`, so this
/// exercises the whole chain the exit code is a contract about: provider →
/// pipeline abort (ADR-0017) → `ExitCode::for_pipeline_failure` → process
/// status. A unit test over the mapping function alone would pass with the
/// pipeline swallowing the error or the CLI never calling the table.
///
/// The guard is returned rather than dropped here so the caller can still
/// see (or not see) `out.md` before the directory removes itself.
///
/// TRACE: ti e62b59
#[cfg(feature = "test-stub-provider")]
fn run_with_stub_mode(mode: &str) -> (Option<i32>, ScratchDir) {
    let workdir = ScratchDir::new(&format!("transync-cli-mode-{mode}"));
    let status = Command::new(bin())
        .env("TRANSYNC_STUB_MODE", mode)
        .arg("translate")
        .arg("--input")
        .arg(fixture_path())
        .arg("--output")
        .arg(workdir.join("out.md"))
        .arg("--map")
        .arg(workdir.join("out.json"))
        .arg("--target-language")
        .arg("ko")
        .status()
        .expect("transync binary must run");
    (status.code(), workdir)
}

/// Exit code 6 — the provider refused the run because of how it was
/// configured. Four causes, one remediation ("fix the configuration and
/// re-run"), so one code.
///
/// The `--input` and every argument are valid in all four runs, which is the
/// distinction from exit `1`: these are not arguments the CLI refused, they
/// are arguments the provider refused.
///
/// TRACE: contracts.md §6
/// TRACE: ti e62b59
#[cfg(feature = "test-stub-provider")]
#[test]
fn cli_exit_6_configuration_rejected() {
    for mode in [
        "auth",
        "provider-rejected",
        "output-ceiling",
        "context-window",
    ] {
        let (code, workdir) = run_with_stub_mode(mode);
        assert_eq!(
            code,
            Some(6),
            "TRANSYNC_STUB_MODE={mode} should exit 6, got {code:?}"
        );
        // ADR-0017: a terminal provider error aborts the run and publishes
        // nothing. The exit code would be a trap if a partly-translated
        // document had already landed under it.
        let out_md = workdir.join("out.md");
        assert!(
            !out_md.exists(),
            "TRANSYNC_STUB_MODE={mode} aborted the run, so {} must not exist",
            out_md.display()
        );
    }
}

/// Exit code 7 — the provider refused this document's content. Distinct
/// from `6` because no configuration change helps, and distinct from `3`
/// because `3` still writes its outputs.
///
/// TRACE: contracts.md §6
/// TRACE: ti e62b59
#[cfg(feature = "test-stub-provider")]
#[test]
fn cli_exit_7_document_refused() {
    for mode in ["content-filtered", "model-refused"] {
        let (code, workdir) = run_with_stub_mode(mode);
        assert_eq!(
            code,
            Some(7),
            "TRANSYNC_STUB_MODE={mode} should exit 7, got {code:?}"
        );
        let out_md = workdir.join("out.md");
        assert!(
            !out_md.exists(),
            "TRANSYNC_STUB_MODE={mode} aborted the run, so {} must not exist",
            out_md.display()
        );
    }
}

/// Exit code 5 stays the residual. Pinned end-to-end alongside `6` and `7`
/// because that is the claim the split rests on: a cause the taxonomy does
/// not classify must not drift into one of the new codes, which would make
/// them mean "probably" instead of "definitely".
///
/// TRACE: contracts.md §6
/// TRACE: ti e62b59
#[cfg(feature = "test-stub-provider")]
#[test]
fn cli_exit_5_stays_the_unclassified_residual() {
    let (code, _) = run_with_stub_mode("unclassified");
    assert_eq!(
        code,
        Some(5),
        "an unclassified provider failure should exit 5, got {code:?}"
    );
}

/// A misspelled `TRANSYNC_STUB_MODE` fails loudly instead of running a
/// green echo translation. Without this the exit-code tests above could pass
/// vacuously the day a mode name is renamed.
///
/// TRACE: ti e62b59
#[cfg(feature = "test-stub-provider")]
#[test]
fn cli_unknown_stub_mode_is_an_error() {
    let (code, workdir) = run_with_stub_mode("no-such-mode");
    assert_eq!(
        code,
        Some(1),
        "an unknown stub mode should be an argument error, got {code:?}"
    );
    assert!(
        !workdir.join("out.md").exists(),
        "no run happened, so no output may exist"
    );
}

/// OI-0012: `--validation-report <path>` writes the per-unit report as
/// JSON (attempt log + final status per unit), committed alongside the
/// other outputs.
#[cfg(feature = "test-stub-provider")]
#[test]
fn cli_validation_report_output() {
    let workdir = ScratchDir::new("transync-cli-valreport");
    let out_md = workdir.join("out.md");
    let out_json = workdir.join("out.json");
    let report = workdir.join("validation.json");

    let status = Command::new(bin())
        .arg("translate")
        .arg("--input")
        .arg(fixture_path())
        .arg("--output")
        .arg(&out_md)
        .arg("--map")
        .arg(&out_json)
        .arg("--validation-report")
        .arg(&report)
        .arg("--target-language")
        .arg("ko")
        .status()
        .expect("transync binary must run");
    assert!(status.success(), "exit code: {:?}", status.code());

    let raw = std::fs::read_to_string(&report).expect("validation report written");
    let parsed: serde_json::Value = serde_json::from_str(&raw).expect("report is JSON");
    // R0001-0027: the published artifact declares its own schema version
    // (contracts.md §3a). Pinned as a literal on purpose — this is the wire,
    // the bytes a downstream reader parses, so the pin has to be independent of
    // whatever constant the linked engine holds. The constant is a separate
    // statement and is on the curated surface as
    // `transync::VALIDATION_REPORT_SCHEMA_VERSION` (§0 tier a, ti 8a4a32): it
    // says what *this* build emits, the literal below says what the CLI
    // actually wrote to disk. Bumping the report version is therefore two
    // edits, `validate.rs` and this literal — deliberately, and the pin
    // immediately below is what turns "only the first one happened" into a
    // named failure instead of a confusing wire mismatch.
    assert_eq!(
        transync::VALIDATION_REPORT_SCHEMA_VERSION,
        "1.1.0",
        "the report schema version moved in validate.rs: bump this smoke's wire \
         literal to match, so the wire stays pinned end-to-end"
    );
    assert_eq!(
        parsed["schema_version"], "1.1.0",
        "the report must carry a version discriminator: {parsed}"
    );
    let per_unit = parsed["per_unit"]
        .as_array()
        .expect("per_unit array present");
    assert!(!per_unit.is_empty(), "per_unit should not be empty");
    for record in per_unit {
        assert!(record["unit_id"].is_string(), "unit_id present: {record}");
        assert!(record["final_status"].is_string(), "final_status present");
        assert!(record["attempts"].is_array(), "attempts array present");
    }
    assert!(parsed["provider_retries"].is_number());
    assert!(parsed["full_reparse_fallbacks"].is_array());
}

/// EXT-2026-07 P1-7: `--max-input-bytes` refuses an input larger than the
/// cap before parsing. Exit 2, stderr names the flag and the limit, and no
/// output is written.
#[cfg(feature = "test-stub-provider")]
#[test]
fn cli_max_input_bytes_rejects_oversized_input() {
    let workdir = ScratchDir::new("transync-cli-maxinput");
    let out_md = workdir.join("out.md");
    let out_json = workdir.join("out.json");

    // The SCN-14 fixture is comfortably larger than 32 bytes.
    let output = Command::new(bin())
        .arg("translate")
        .arg("--input")
        .arg(fixture_path())
        .arg("--output")
        .arg(&out_md)
        .arg("--map")
        .arg(&out_json)
        .arg("--target-language")
        .arg("ko")
        .arg("--max-input-bytes")
        .arg("32")
        .output()
        .expect("transync binary must run");

    assert_eq!(
        output.status.code(),
        Some(2),
        "oversized input should exit 2, got {:?}",
        output.status.code()
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--max-input-bytes"),
        "stderr must name the flag: {stderr}"
    );
    assert!(
        stderr.contains("32"),
        "stderr must name the byte limit: {stderr}"
    );
    assert!(
        !out_md.exists() && !out_json.exists(),
        "admission failure must not write any output"
    );
}

/// ti `13e145`: an `--input` whose preamble declares an HTML document is
/// refused at the boundary. Exit 2 — the same code the other admission
/// refusals use — the message names both escapes (`--input-format html` and
/// `--allow-html-input`), and nothing is written.
#[cfg(feature = "test-stub-provider")]
#[test]
fn cli_html_document_input_is_refused() {
    let workdir = ScratchDir::new("transync-cli-htmlinput");
    let input = workdir.join("page.html");
    std::fs::write(&input, HTML_DOCUMENT).unwrap();
    let out_md = workdir.join("out.md");
    let out_json = workdir.join("out.json");

    let output = Command::new(bin())
        .arg("translate")
        .arg("--input")
        .arg(&input)
        .arg("--output")
        .arg(&out_md)
        .arg("--map")
        .arg(&out_json)
        .arg("--target-language")
        .arg("ko")
        .output()
        .expect("transync binary must run");

    assert_eq!(
        output.status.code(),
        Some(2),
        "an HTML document should exit 2, got {:?}",
        output.status.code()
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--allow-html-input"),
        "stderr must name the override flag: {stderr}"
    );
    assert!(
        stderr.contains("--input-format html"),
        "stderr must point at the HTML-document path that now exists: {stderr}"
    );
    assert!(
        !stderr.contains("unimplemented") && !stderr.contains("not implemented"),
        "the feature exists; the refusal must not lie in that direction: {stderr}"
    );
    assert!(
        !out_md.exists() && !out_json.exists(),
        "a refused input must not write any output"
    );
}

/// The other half of the same guard: `--allow-html-input` restores the
/// previous behavior exactly, so the refusal is a boundary decision the
/// operator can overrule rather than a capability that was removed.
#[cfg(feature = "test-stub-provider")]
#[test]
fn cli_allow_html_input_forces_the_previous_behavior() {
    let workdir = ScratchDir::new("transync-cli-htmlinput-allow");
    let input = workdir.join("page.html");
    std::fs::write(&input, HTML_DOCUMENT).unwrap();
    let out_md = workdir.join("out.md");
    let out_json = workdir.join("out.json");

    let output = Command::new(bin())
        .arg("translate")
        .arg("--input")
        .arg(&input)
        .arg("--output")
        .arg(&out_md)
        .arg("--map")
        .arg(&out_json)
        .arg("--target-language")
        .arg("ko")
        .arg("--allow-html-input")
        .output()
        .expect("transync binary must run");

    assert_eq!(
        output.status.code(),
        Some(0),
        "the override must translate it as Markdown, got {:?}: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr),
    );
    assert!(
        out_md.exists() && out_json.exists(),
        "the override run must publish its outputs"
    );
}

/// ti `d990b6`, re-stated for ti `457e51`: what an indented run in HTML input
/// *actually* does, welded to the refusal message that describes it.
///
/// The original pin recorded a defect. Six texts said the indented line "comes
/// back fenced"; it did not, because a code block's unit payload was the
/// **dedented** body, which reparses as a paragraph — so
/// `validate::fragment_reparse` rejected every attempt and the block settled
/// as `fallback_source` with its source bytes spliced back verbatim. That
/// test's own doc said a change which "really did fence an indented run would
/// fail this test rather than quietly make the old claim true again". ti
/// `457e51` is that change, and this is the test failing and being re-stated
/// rather than the claim drifting back silently.
///
/// The new truth, and the reason this is still a pin worth having: the run is
/// *translated* now, and `regen::regenerate_code_block` IS reached, so the
/// document that comes out of an HTML-input run has a different shape from the
/// one that went in — a fenced block where the source had an indented one.
/// Submitting an HTML document as Markdown is still wrong; it is now wrong by
/// reshaping rather than by omitting, and nothing in the map flags it, which
/// is exactly why the `--allow-html-input` refusal exists.
#[cfg(feature = "test-stub-provider")]
#[test]
fn cli_html_input_re_emits_an_indented_run_as_a_fenced_block() {
    let workdir = ScratchDir::new("transync-cli-htmlinput-indented");
    let input = workdir.join("page.html");
    std::fs::write(&input, HTML_DOCUMENT).unwrap();
    let out_md = workdir.join("out.md");
    let out_json = workdir.join("out.json");

    let output = Command::new(bin())
        .arg("translate")
        .arg("--input")
        .arg(&input)
        .arg("--output")
        .arg(&out_md)
        .arg("--map")
        .arg(&out_json)
        .arg("--target-language")
        .arg("ko")
        .arg("--allow-html-input")
        .output()
        .expect("transync binary must run");
    assert_eq!(
        output.status.code(),
        Some(0),
        "the override run completes: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let out = std::fs::read_to_string(&out_md).expect("out.md should be readable");
    assert!(
        out.contains("\n```\nA four-space-indented line.\n```\n"),
        "the indented run comes back as a bare-fenced block, dedented and at \
         column 0: {out:?}"
    );
    assert!(
        !out.contains("    A four-space-indented line."),
        "the indented spelling is gone — that is the reshaping: {out:?}"
    );
    assert!(
        !out.contains("    ```"),
        "no indent may survive ahead of the fence; an indented fence marker \
         would re-open an indented block and leave the real fence unclosed: \
         {out:?}"
    );

    let alignment: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&out_json).expect("map readable"))
            .expect("alignment JSON should parse");
    let blocks = alignment["blocks"].as_array().expect("blocks array");
    let code = blocks
        .iter()
        .find(|b| b["block_kind"].as_str() == Some("code-block"))
        .expect("the indented run became a code block, which is the premise");
    assert_eq!(
        code["fallback_status"].as_str(),
        Some("translated"),
        "it translates on the first attempt now: {code}"
    );
    assert_eq!(
        alignment["validation_summary"]["fallback_source"].as_u64(),
        Some(0),
        "nothing falls back, so nothing in the map marks the reshaping — the \
         `--allow-html-input` refusal is the only thing that does"
    );
}

/// A Markdown document that opens with a raw-HTML island is NOT an HTML
/// document, and must run without the flag. The refusal's false-positive
/// direction is the expensive one — it stops a run that would have been
/// correct — so it is pinned end to end, not only in the sniff's unit tests.
#[cfg(feature = "test-stub-provider")]
#[test]
fn cli_markdown_opening_with_an_html_island_is_not_refused() {
    let workdir = ScratchDir::new("transync-cli-htmlinput-island");
    let input = workdir.join("README.md");
    std::fs::write(
        &input,
        "<div align=\"center\">\n<b>Hero banner</b>\n</div>\n\n# Project\n\nProse.\n",
    )
    .unwrap();
    let out_md = workdir.join("out.md");
    let out_json = workdir.join("out.json");

    let output = Command::new(bin())
        .arg("translate")
        .arg("--input")
        .arg(&input)
        .arg("--output")
        .arg(&out_md)
        .arg("--map")
        .arg(&out_json)
        .arg("--target-language")
        .arg("ko")
        .output()
        .expect("transync binary must run");

    assert_eq!(
        output.status.code(),
        Some(0),
        "HTML islands are supported input (ADR-0018), got {:?}: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr),
    );
}

/// EXT-2026-07 P1-6: `--out-dir` publishes the full output set into a fresh
/// directory — out.md, alignment.json, validation-report.json, and the
/// six-file html/ bundle — leaving no staging/backup residue behind.
#[cfg(feature = "test-stub-provider")]
#[test]
fn cli_out_dir_fresh_publish() {
    let workdir = ScratchDir::new("transync-cli-outdir-fresh");
    let out_dir = workdir.join("published");

    let status = Command::new(bin())
        .arg("translate")
        .arg("--input")
        .arg(fixture_path())
        .arg("--out-dir")
        .arg(&out_dir)
        .arg("--target-language")
        .arg("ko")
        .status()
        .expect("transync binary must run");
    assert!(status.success(), "exit code: {:?}", status.code());

    for rel in [
        "out.md",
        "alignment.json",
        "validation-report.json",
        "html/index.html",
        "html/source.html",
        "html/target.html",
        "html/alignment.json",
        "html/sync.js",
        "html/purify.min.js",
    ] {
        let path = out_dir.join(rel);
        let meta = std::fs::metadata(&path)
            .unwrap_or_else(|e| panic!("expected {} to exist: {}", path.display(), e));
        assert!(meta.len() > 0, "expected {} non-empty", path.display());
    }
    assert_no_staging_residue(&workdir, "published");
}

/// EXT-2026-07 P1-6: republishing over an existing prior-transync out-dir
/// succeeds without `--force` (the backup/swap dance), and the second run's
/// content wins.
#[cfg(feature = "test-stub-provider")]
#[test]
fn cli_out_dir_republish_over_existing() {
    let workdir = ScratchDir::new("transync-cli-outdir-republish");
    let out_dir = workdir.join("published");

    let run = || {
        Command::new(bin())
            .arg("translate")
            .arg("--input")
            .arg(fixture_path())
            .arg("--out-dir")
            .arg(&out_dir)
            .arg("--target-language")
            .arg("ko")
            .status()
            .expect("transync binary must run")
    };

    assert!(run().success(), "first publish should succeed");
    // Overwrite the published out.md so the second publish must prove its
    // fresh content wins (the whole directory is swapped, not merged).
    std::fs::write(out_dir.join("out.md"), b"STALE").unwrap();
    assert!(run().success(), "republish should succeed without --force");

    let out_md = std::fs::read(out_dir.join("out.md")).expect("out.md readable");
    assert_ne!(out_md, b"STALE", "republish must overwrite prior content");
    assert!(!out_md.is_empty(), "out.md non-empty after republish");
    assert!(
        out_dir.join("html/index.html").exists(),
        "bundle present after republish"
    );
    assert_no_staging_residue(&workdir, "published");
}

/// ti `66339b`: a staging temp at the TOP LEVEL of an `--out-dir` target is
/// transync's own crash residue — a `--output <dir>/out.md` run that died
/// mid-stage leaves one — and used to make the whole target read as foreign,
/// so the operator was told to pass `--force` over transync's own leftovers
/// (exit 4). The republish now succeeds, and the leftover goes the way of
/// everything else in the replaced tree.
#[cfg(feature = "test-stub-provider")]
#[test]
fn cli_out_dir_tolerates_a_top_level_staging_temp() {
    let workdir = ScratchDir::new("transync-cli-outdir-temp");
    let out_dir = workdir.join("published");

    let run = || {
        Command::new(bin())
            .arg("translate")
            .arg("--input")
            .arg(fixture_path())
            .arg("--out-dir")
            .arg(&out_dir)
            .arg("--target-language")
            .arg("ko")
            .status()
            .expect("transync binary must run")
    };

    assert!(run().success(), "first publish should succeed");
    let leftover = out_dir.join("out.md.tmp.2147483646");
    std::fs::write(&leftover, b"a crashed run's staging").unwrap();

    let status = run();
    assert!(
        status.success(),
        "transync's own staging residue must not demand --force, got {:?}",
        status.code()
    );
    assert!(
        !leftover.exists(),
        "the replaced tree takes the leftover with it"
    );
    assert!(
        out_dir.join("out.md").exists() && out_dir.join("html/index.html").exists(),
        "the republished set must be complete"
    );
    assert_no_staging_residue(&workdir, "published");
}

/// EXT-2026-07 P1-6: `--out-dir` conflicts with `--output` (clap rejects it
/// at parse time → exit 1).
#[cfg(feature = "test-stub-provider")]
#[test]
fn cli_out_dir_conflicts_with_output() {
    let workdir = ScratchDir::new("transync-cli-outdir-conflict");
    let status = Command::new(bin())
        .arg("translate")
        .arg("--input")
        .arg(fixture_path())
        .arg("--out-dir")
        .arg(workdir.join("published"))
        .arg("--output")
        .arg(workdir.join("out.md"))
        .arg("--target-language")
        .arg("ko")
        .status()
        .expect("transync binary must run");
    assert_eq!(
        status.code(),
        Some(1),
        "--out-dir with --output should exit 1, got {:?}",
        status.code()
    );
}

/// EXT-2026-07 review-fix: a user file nested inside `<out-dir>/html/` must
/// block a no-force republish (exit 4) and survive it — a successful swap
/// `remove_dir_all`s the whole prior target, so the top-level allow-list is
/// not enough. With `--force` the publish proceeds.
#[cfg(feature = "test-stub-provider")]
#[test]
fn cli_out_dir_refuses_foreign_file_nested_in_html() {
    let workdir = ScratchDir::new("transync-cli-outdir-nested");
    let out_dir = workdir.join("published");

    let run = |force: bool| {
        let mut cmd = Command::new(bin());
        cmd.arg("translate")
            .arg("--input")
            .arg(fixture_path())
            .arg("--out-dir")
            .arg(&out_dir)
            .arg("--target-language")
            .arg("ko");
        if force {
            cmd.arg("--force");
        }
        cmd.status().expect("transync binary must run")
    };

    // Fresh publish creates the html/ bundle directory.
    assert!(run(false).success(), "first publish should succeed");

    // Plant a user file nested inside html/.
    let foreign = out_dir.join("html").join("USER_NOTES.txt");
    std::fs::write(&foreign, b"keepme").unwrap();

    // Republish WITHOUT --force must refuse (exit 4) and leave the file intact.
    let status = run(false);
    assert_eq!(
        status.code(),
        Some(4),
        "nested foreign file should force exit 4, got {:?}",
        status.code()
    );
    let survived =
        std::fs::read(&foreign).expect("nested user file must survive a refused republish");
    assert_eq!(
        survived,
        b"keepme".to_vec(),
        "nested user file content must be intact after refusal"
    );

    // With --force the publish proceeds (the whole tree is replaced).
    assert!(run(true).success(), "republish with --force should succeed");
    assert_no_staging_residue(&workdir, "published");
}

/// OI-0023 item 1: `--force` overwrite semantics for the `--html-out` bundle
/// directory. A foreign (non-bundle) file makes the directory unsafe to
/// overwrite: the publish REFUSES without `--force` (exit 4 — WriteFailure —
/// leaving the foreign file and writing no `--output`/`--map`) and OVERWRITES
/// with `--force` (the bundle is written despite the foreign file). Modeled on
/// the `--out-dir` foreign-file guard test above.
#[cfg(feature = "test-stub-provider")]
#[test]
fn cli_html_out_force_overwrites_foreign_file() {
    let workdir = ScratchDir::new("transync-cli-htmlout-force");
    let out_md = workdir.join("out.md");
    let out_json = workdir.join("out.json");
    let html_out = workdir.join("html");

    // Plant a foreign (non-bundle) file so the html-out dir is not a clean
    // prior-transync bundle.
    std::fs::create_dir_all(&html_out).unwrap();
    let foreign = html_out.join("USER_NOTES.txt");
    std::fs::write(&foreign, b"keepme").unwrap();

    let run = |force: bool| {
        let mut cmd = Command::new(bin());
        cmd.arg("translate")
            .arg("--input")
            .arg(fixture_path())
            .arg("--output")
            .arg(&out_md)
            .arg("--map")
            .arg(&out_json)
            .arg("--html-out")
            .arg(&html_out)
            .arg("--target-language")
            .arg("ko");
        if force {
            cmd.arg("--force");
        }
        cmd.status().expect("transync binary must run")
    };

    // Without --force: refuse (exit 4). The foreign file survives and the
    // preflight runs before any output write, so --output/--map are untouched.
    let status = run(false);
    assert_eq!(
        status.code(),
        Some(4),
        "foreign --html-out file should force exit 4, got {:?}",
        status.code()
    );
    assert_eq!(
        std::fs::read(&foreign).expect("foreign file must survive a refused write"),
        b"keepme".to_vec(),
        "foreign file content must be intact after refusal"
    );
    assert!(
        !out_md.exists() && !out_json.exists(),
        "a refused --html-out preflight must not write --output/--map"
    );

    // With --force: the guard is bypassed, so the write proceeds and the full
    // six-file bundle plus --output/--map are all written. --html-out is a
    // per-file overwrite (write_fileset_atomic renames the six bundle names
    // over their targets), NOT a whole-tree replace like --out-dir, so the
    // planted foreign file survives alongside the bundle.
    assert!(run(true).success(), "republish with --force should succeed");
    for entry in [
        "index.html",
        "source.html",
        "target.html",
        "alignment.json",
        "sync.js",
        "purify.min.js",
    ] {
        assert!(
            html_out.join(entry).exists(),
            "bundle file {entry} must be written under --force"
        );
    }
    assert!(
        out_md.exists() && out_json.exists(),
        "--output/--map must be written under --force"
    );
    assert_eq!(
        std::fs::read(&foreign).expect("foreign file must survive an --html-out overwrite"),
        b"keepme".to_vec(),
        "--html-out --force is a per-file overwrite, not a tree replace: the foreign file survives"
    );
}

/// OI-0023 item 2: the `--model` and `--base-url` CLI flags reach the provider.
/// With the `test-stub-provider` feature there is no live provider to inspect,
/// so the stub provider records the model + base-url it was constructed with —
/// the same pair the live build forwards to `TransyncOpenAI::try_new` — to the
/// path in `TRANSYNC_STUB_ECHO_PATH`. Reading it back proves the flags
/// propagated through the CLI boundary.
#[cfg(feature = "test-stub-provider")]
#[test]
fn cli_model_and_base_url_reach_provider() {
    let workdir = ScratchDir::new("transync-cli-provider-wiring");
    let echo = workdir.join("provider.json");

    let status = Command::new(bin())
        .env("TRANSYNC_STUB_ECHO_PATH", &echo)
        .arg("translate")
        .arg("--input")
        .arg(fixture_path())
        .arg("--output")
        .arg(workdir.join("out.md"))
        .arg("--map")
        .arg(workdir.join("out.json"))
        .arg("--target-language")
        .arg("ko")
        .arg("--model")
        .arg("propagation-probe-model")
        .arg("--base-url")
        .arg("https://stub.example/v1")
        .status()
        .expect("transync binary must run");
    assert!(status.success(), "exit code: {:?}", status.code());

    let raw = std::fs::read_to_string(&echo).expect("provider echo should be written");
    let parsed: serde_json::Value = serde_json::from_str(&raw).expect("provider echo is JSON");
    assert_eq!(
        parsed["model"].as_str(),
        Some("propagation-probe-model"),
        "the --model flag must reach the provider: {parsed}"
    );
    assert_eq!(
        parsed["base_url"].as_str(),
        Some("https://stub.example/v1"),
        "the --base-url flag must reach the provider: {parsed}"
    );
}

/// R0001-0034 / DCR-0021: two CLI *processes* aimed at the same
/// `--output` / `--map` / `--html-out` set serialize their publication instead
/// of interleaving their rename passes. Each run is given a different
/// `--target-language`, which is stamped into three separately-renamed
/// artifacts (the map, the bundle's copy of the map, and `index.html`'s `lang`
/// attributes) — an interleaved phase 2 leaves them disagreeing. Both runs
/// must also succeed: the publish lock queues the loser rather than failing it.
#[cfg(feature = "test-stub-provider")]
#[test]
fn cli_concurrent_processes_publish_one_run_not_a_mixture() {
    let workdir = ScratchDir::new("transync-cli-concurrent-publish");
    let out_md = workdir.join("out.md");
    let out_json = workdir.join("out.json");
    let html_out = workdir.join("html");

    let mut running = Vec::new();
    for language in ["ko", "de"] {
        running.push(
            Command::new(bin())
                .arg("translate")
                .arg("--input")
                .arg(fixture_path())
                .arg("--output")
                .arg(&out_md)
                .arg("--map")
                .arg(&out_json)
                .arg("--html-out")
                .arg(&html_out)
                .arg("--target-language")
                .arg(language)
                .arg("--quiet")
                .spawn()
                .expect("transync binary must run"),
        );
    }
    for mut child in running {
        let status = child.wait().expect("child should finish");
        assert!(
            status.success(),
            "a run queued behind another must still succeed, got {:?}",
            status.code()
        );
    }

    let published_language = |path: &std::path::Path| -> String {
        let raw = std::fs::read_to_string(path).expect("map readable");
        let parsed: serde_json::Value = serde_json::from_str(&raw).expect("map is JSON");
        parsed["target_language"]
            .as_str()
            .expect("map carries the target language")
            .to_string()
    };

    let winner = published_language(&out_json);
    assert_eq!(
        published_language(&html_out.join("alignment.json")),
        winner,
        "the bundle's map is from a different run than --map"
    );
    let index = std::fs::read_to_string(html_out.join("index.html")).expect("index readable");
    assert!(
        index.contains(&format!("lang=\"{winner}\"")),
        "index.html is from a different run than the alignment map ({winner})"
    );
    let loser = if winner == "ko" { "de" } else { "ko" };
    assert!(
        !index.contains(&format!("lang=\"{loser}\"")),
        "index.html carries both runs' languages"
    );

    // The lock markers are transync's own files and are deliberately left in
    // place (DCR-0021); nothing else may survive the publication.
    for dir in [workdir.path(), html_out.as_path()] {
        for entry in std::fs::read_dir(dir).expect("dir readable") {
            let name = entry.expect("entry readable").file_name();
            let name = name.to_string_lossy();
            assert!(
                !name.contains(".tmp."),
                "a staging temp survived the publication: {name}"
            );
        }
    }
    assert!(
        html_out.join(".transync-publish.lock").exists(),
        "the publish lock marker lives in every directory published into"
    );
}

/// Assert the publish left no `.<name>.staging.<pid>` / `.<name>.backup.<pid>`
/// siblings in `parent`. EXT-2026-07 P1-6.
#[cfg(feature = "test-stub-provider")]
fn assert_no_staging_residue(parent: &std::path::Path, name: &str) {
    let staging_prefix = format!(".{name}.staging.");
    let backup_prefix = format!(".{name}.backup.");
    for entry in std::fs::read_dir(parent).expect("parent readable") {
        let entry = entry.expect("entry readable");
        let fname = entry.file_name();
        let fname = fname.to_string_lossy();
        assert!(
            !fname.starts_with(&staging_prefix) && !fname.starts_with(&backup_prefix),
            "unexpected publish residue: {fname}"
        );
    }
}

/// D1 §2.3 / test #9: `--target-output-tokens 100 --output-expansion-factor 5.0`
/// drives the whole fixture over the tiny output ceiling. The stub echoes the
/// source (ignoring ceilings) so the run succeeds, the validation report JSON
/// carries an `output_budget_warnings` entry naming a block, and stderr carries
/// the warning plus the remediation flag.
///
/// R0003-0034: the ceiling has to clear the 64-token response-envelope reserve
/// to be a ceiling at all — a smaller one is warned about and ignored, which
/// would leave nothing to flag. It is still tiny next to the fixture.
#[cfg(feature = "test-stub-provider")]
#[test]
fn cli_output_budget_warning_flags_block() {
    let workdir = ScratchDir::new("transync-cli-budget-warn");
    let out_md = workdir.join("out.md");
    let out_json = workdir.join("out.json");
    let report = workdir.join("r.json");

    let output = Command::new(bin())
        .arg("translate")
        .arg("--input")
        .arg(fixture_path())
        .arg("--output")
        .arg(&out_md)
        .arg("--map")
        .arg(&out_json)
        .arg("--validation-report")
        .arg(&report)
        .arg("--target-language")
        .arg("ko")
        .arg("--target-output-tokens")
        .arg("100")
        .arg("--output-expansion-factor")
        .arg("5.0")
        .output()
        .expect("transync binary must run");

    assert!(
        output.status.success(),
        "run should succeed (stub ignores ceilings): {:?}",
        output.status.code()
    );

    let raw = std::fs::read_to_string(&report).expect("validation report written");
    let parsed: serde_json::Value = serde_json::from_str(&raw).expect("report is JSON");
    let warns = parsed["output_budget_warnings"]
        .as_array()
        .expect("output_budget_warnings array present");
    assert!(
        !warns.is_empty(),
        "at least one over-ceiling block should be flagged: {parsed}"
    );
    let first = &warns[0];
    assert!(
        first["unit_id"].is_string(),
        "the warning names a block id: {first}"
    );
    assert_eq!(
        first["ceiling_tokens"].as_u64(),
        Some(100),
        "reports the raw ceiling, not the reserved target: {first}"
    );
    assert!(
        first["estimated_output_tokens"].as_u64().is_some(),
        "carries an estimate: {first}"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("warning:"),
        "stderr must carry the warning line: {stderr}"
    );
    assert!(
        stderr.contains("--target-output-tokens"),
        "stderr must name the remediation flag: {stderr}"
    );
}

/// D1 §2.4 / test #10: `--target-output-tokens 0` disables the output ceiling
/// (the `0` sentinel beats the default profile's 8000), so output-aware packing
/// and the preflight are off — the report carries no `output_budget_warnings`.
#[cfg(feature = "test-stub-provider")]
#[test]
fn cli_target_output_tokens_zero_disables_warnings() {
    let workdir = ScratchDir::new("transync-cli-budget-off");
    let out_md = workdir.join("out.md");
    let out_json = workdir.join("out.json");
    let report = workdir.join("r.json");

    let status = Command::new(bin())
        .arg("translate")
        .arg("--input")
        .arg(fixture_path())
        .arg("--output")
        .arg(&out_md)
        .arg("--map")
        .arg(&out_json)
        .arg("--validation-report")
        .arg(&report)
        .arg("--target-language")
        .arg("ko")
        .arg("--target-output-tokens")
        .arg("0")
        // A large factor would flag everything if the ceiling were live; it is
        // not, because 0 turns the whole output cap off.
        .arg("--output-expansion-factor")
        .arg("5.0")
        .status()
        .expect("transync binary must run");
    assert!(status.success(), "exit code: {:?}", status.code());

    let raw = std::fs::read_to_string(&report).expect("validation report written");
    let parsed: serde_json::Value = serde_json::from_str(&raw).expect("report is JSON");
    let warns = parsed["output_budget_warnings"]
        .as_array()
        .expect("output_budget_warnings array present");
    assert!(
        warns.is_empty(),
        "disabling the ceiling must silence the preflight entirely: {parsed}"
    );
}

/// OI-0032 helper: the generated `index.html` line declaring one pane div.
#[cfg(feature = "test-stub-provider")]
fn pane_line<'a>(index_html: &'a str, pane_id: &str) -> &'a str {
    let needle = format!("id=\"{pane_id}\"");
    index_html
        .lines()
        .find(|l| l.contains(&needle))
        .unwrap_or_else(|| panic!("index.html must declare a {pane_id} pane"))
}

/// OI-0032 helper: run `translate --html-out` over the SCN-14 fixture with
/// extra flags and return the generated `index.html`.
#[cfg(feature = "test-stub-provider")]
fn bundle_index_html(prefix: &str, target_language: &str, extra: &[&str]) -> String {
    bundle_index_html_for(prefix, &fixture_path(), target_language, extra)
}

/// ti 0f26b5 helper: as [`bundle_index_html`], but over a caller-written input
/// document — the title's middle precedence level is a property of the
/// *document*, so it needs inputs the fixture cannot express (no H1 at all, a
/// heading carrying markup characters).
#[cfg(feature = "test-stub-provider")]
fn bundle_index_html_for(
    prefix: &str,
    input: &std::path::Path,
    target_language: &str,
    extra: &[&str],
) -> String {
    let workdir = ScratchDir::new(prefix);
    let html_out = workdir.join("html");
    let mut cmd = Command::new(bin());
    cmd.arg("translate")
        .arg("--input")
        .arg(input)
        .arg("--output")
        .arg(workdir.join("out.md"))
        .arg("--map")
        .arg(workdir.join("out.json"))
        .arg("--html-out")
        .arg(&html_out)
        .arg("--target-language")
        .arg(target_language);
    for a in extra {
        cmd.arg(a);
    }
    let status = cmd.status().expect("transync binary must run");
    assert!(
        status.success(),
        "translate --target-language {target_language} {extra:?} exit code: {:?}",
        status.code()
    );
    std::fs::read_to_string(html_out.join("index.html")).expect("index.html should be readable")
}

/// ti 0f26b5 helper: write one input document into its own scratch dir.
///
/// The scratch guard rides along inside the returned value rather than being
/// dropped here, so the document is on disk for exactly as long as the caller
/// holds the handle. Returning a bare path would hand back the name of a file
/// this function had already deleted.
#[cfg(feature = "test-stub-provider")]
fn input_document(prefix: &str, markdown: &str) -> InputDocument {
    let workdir = ScratchDir::new(prefix);
    let path = workdir.join("input.md");
    std::fs::write(&path, markdown.as_bytes()).expect("input document should be writable");
    InputDocument {
        path,
        _dir: workdir,
    }
}

/// A written input document and the scratch directory that holds it; derefs
/// to the document's path.
#[cfg(feature = "test-stub-provider")]
struct InputDocument {
    path: std::path::PathBuf,
    _dir: ScratchDir,
}

#[cfg(feature = "test-stub-provider")]
impl std::ops::Deref for InputDocument {
    type Target = std::path::Path;

    fn deref(&self) -> &std::path::Path {
        &self.path
    }
}

/// ti 0f26b5: the emitted bundle carries a real document title and the run's
/// target language, through all three precedence levels — `--title` beats the
/// document's first H1, the H1 titles a run that passes no flag, and a
/// document with no H1 falls back to the literal `transync` every pre-ticket
/// bundle carried. `<html lang>` is asserted alongside because the two slots
/// are the pair this ticket closes: a bundle that knows what the document is
/// called and what language it is in.
#[cfg(feature = "test-stub-provider")]
#[test]
fn cli_bundle_title_resolves_flag_then_first_h1_then_literal() {
    let from_h1 = bundle_index_html("transync-cli-title-h1", "ko", &[]);
    assert!(
        from_h1.contains("<title>transync — full-coverage smoke fixture</title>"),
        "with no flag the fixture's first H1 titles the bundle: {from_h1}"
    );
    assert!(
        from_h1.contains("<html lang=\"ko\">"),
        "the document language is the run's target language: {from_h1}"
    );

    let from_flag = bundle_index_html(
        "transync-cli-title-flag",
        "ko",
        &["--title", "Release Notes"],
    );
    assert!(
        from_flag.contains("<title>Release Notes</title>"),
        "--title must beat the document's own heading: {from_flag}"
    );

    let no_h1 = input_document(
        "transync-cli-title-no-h1",
        "## Only a subheading\n\nBody text with no level-1 heading.\n",
    );
    let fallback = bundle_index_html_for("transync-cli-title-none", &no_h1, "ko", &[]);
    assert!(
        fallback.contains("<title>transync</title>"),
        "a document with no H1 keeps the literal fallback: {fallback}"
    );
}

/// ti 0f26b5 / invariant 7: the title's middle precedence level is untrusted
/// source content, so it reaches `<title>` escaped. The code span is the path
/// that matters — inline raw HTML is markup the parser drops, but a code
/// span's literal is *text*, so it delivers `<` and `&` into the title.
#[cfg(feature = "test-stub-provider")]
#[test]
fn cli_bundle_title_escapes_an_untrusted_heading() {
    let hostile = input_document(
        "transync-cli-title-hostile",
        "# The `</title><script>alert(1)</script>` case & more\n\nBody.\n",
    );
    let index = bundle_index_html_for("transync-cli-title-hostile-run", &hostile, "ko", &[]);

    assert!(
        index.contains(
            "<title>The &lt;/title&gt;&lt;script&gt;alert(1)&lt;/script&gt; case &amp; more</title>"
        ),
        "an untrusted heading must land in <title> as escaped text: {index}"
    );
    assert!(
        !index.contains("<script>alert(1)"),
        "a heading must not be able to open an element in the shell: {index}"
    );
}

/// ti 0f26b5: an explicitly empty `--title` is a usage error (exit 1), not a
/// request for the fallback — the same posture an explicit empty `--model`
/// gets. It fails before the input is read, so the mistake costs no run.
#[cfg(feature = "test-stub-provider")]
#[test]
fn cli_blank_title_is_an_argument_error() {
    let workdir = ScratchDir::new("transync-cli-title-blank");
    let status = Command::new(bin())
        .arg("translate")
        .arg("--input")
        .arg(workdir.join("never-read.md"))
        .arg("--output")
        .arg(workdir.join("out.md"))
        .arg("--map")
        .arg(workdir.join("out.json"))
        .arg("--target-language")
        .arg("ko")
        .arg("--title")
        .arg("   ")
        .status()
        .expect("transync binary must run");
    assert_eq!(
        status.code(),
        Some(1),
        "a blank --title should exit 1, got {:?}",
        status.code()
    );
}

/// OI-0032: an RTL `--target-language` stamps `dir="rtl"` on the bundle's
/// target pane and leaves the source pane alone; an LTR target emits no `dir`
/// attribute anywhere (byte-stability regression for the shipped default).
#[cfg(feature = "test-stub-provider")]
#[test]
fn cli_rtl_target_language_stamps_target_pane() {
    let arabic = bundle_index_html("transync-cli-dir-ar", "ar", &[]);
    assert!(
        pane_line(&arabic, "target").contains("dir=\"rtl\""),
        "target pane must carry dir=\"rtl\": {}",
        pane_line(&arabic, "target")
    );
    assert!(
        !pane_line(&arabic, "source").contains("dir="),
        "the source pane resolves from its own (LTR) label: {}",
        pane_line(&arabic, "source")
    );
    // The document language still rides on <html lang>, not <html dir>.
    assert!(
        arabic.contains("<html lang=\"ar\">"),
        "<html> keeps only lang"
    );

    let korean = bundle_index_html("transync-cli-dir-ko", "ko", &[]);
    assert!(
        !korean.contains("dir="),
        "an LTR target must emit no dir attribute at all"
    );
}

/// OI-0032: an explicit `--target-direction` overrides the auto subtag table
/// in both directions. `ltr` on an Arabic run renders LTR by *absence* of the
/// attribute (LTR is the HTML default).
#[cfg(feature = "test-stub-provider")]
#[test]
fn cli_target_direction_flag_overrides_table() {
    let forced_ltr = bundle_index_html(
        "transync-cli-dir-ar-ltr",
        "ar",
        &["--target-direction", "ltr"],
    );
    assert!(
        !forced_ltr.contains("dir="),
        "--target-direction ltr must suppress the auto-table RTL attribute"
    );

    let forced_rtl = bundle_index_html(
        "transync-cli-dir-ko-rtl",
        "ko",
        &["--target-direction", "rtl"],
    );
    assert!(
        pane_line(&forced_rtl, "target").contains("dir=\"rtl\""),
        "--target-direction rtl must stamp a target pane the table would not: {}",
        pane_line(&forced_rtl, "target")
    );
    assert!(
        !pane_line(&forced_rtl, "source").contains("dir="),
        "the flag governs the target pane only"
    );
}

/// OI-0032: the profile's `[render].target_direction` applies when no flag is
/// given, and an explicit flag (including `auto`) beats it.
#[cfg(feature = "test-stub-provider")]
#[test]
fn cli_profile_render_direction_applies_and_flag_beats_it() {
    let workdir = ScratchDir::new("transync-cli-dir-profile");
    let profile = workdir.join("rtl-profile.toml");
    std::fs::write(
        &profile,
        b"slug = \"rtl-render\"\nversion = \"1.0.0\"\n\n[system]\nprompt = \"Translate from {{source_language}} to {{target_language}}.\"\n\n[render]\ntarget_direction = \"rtl\"\n",
    )
    .expect("profile TOML should be writable");
    let profile_arg = profile.to_str().expect("profile path is UTF-8");

    let from_profile = bundle_index_html(
        "transync-cli-dir-profile-on",
        "ko",
        &["--profile", profile_arg],
    );
    assert!(
        pane_line(&from_profile, "target").contains("dir=\"rtl\""),
        "[render].target_direction = \"rtl\" must stamp the target pane: {}",
        pane_line(&from_profile, "target")
    );

    let flag_wins = bundle_index_html(
        "transync-cli-dir-profile-override",
        "ko",
        &["--profile", profile_arg, "--target-direction", "auto"],
    );
    assert!(
        !flag_wins.contains("dir="),
        "an explicit --target-direction auto must beat the profile's rtl"
    );
}

/// OI-0018 (posture amended 2026-08-07): the exact `<meta>` element
/// `--strict-csp` injects, leading newline and indent included. Spelled out
/// here rather than imported — `transync-cli` is a binary crate, and a golden
/// the test owns is what makes a silent policy change fail this test.
#[cfg(feature = "test-stub-provider")]
const CSP_META: &str = "\n    <meta http-equiv=\"Content-Security-Policy\" content=\"default-src \
                        'self'; img-src 'self' data:; script-src 'self' 'unsafe-inline'; \
                        style-src 'self' 'unsafe-inline'; connect-src 'self'; object-src 'none'; \
                        base-uri 'none'; form-action 'none'\">";

/// OI-0018: `--strict-csp` puts the policy in the emitted bundle, and without
/// it the bundle is byte-identical to what pre-flag transync wrote. Both halves
/// are asserted against one pair of real runs: `index.html` does not depend on
/// the translated content (only on the language/direction slots, held equal
/// here), so "the strict shell minus the element IS the default shell" is a
/// byte-level statement about the shipped default output.
#[cfg(feature = "test-stub-provider")]
#[test]
fn cli_strict_csp_injects_the_policy_and_leaves_the_default_untouched() {
    let plain = bundle_index_html("transync-cli-csp-off", "ko", &[]);
    let strict = bundle_index_html("transync-cli-csp-on", "ko", &["--strict-csp"]);

    assert!(
        !plain.contains("Content-Security-Policy"),
        "the shipped default posture emits no policy: {plain}"
    );
    assert!(
        strict.contains(CSP_META),
        "--strict-csp must emit the documented policy verbatim: {strict}"
    );
    assert_eq!(
        strict.replace(CSP_META, ""),
        plain,
        "--strict-csp must add the policy element and change nothing else"
    );
}

/// OI-0018: the flag governs whichever bundle the run emits, so `--out-dir`'s
/// `html/` tree gets the same policy — the two modes share one shell
/// assembler and must not drift into a mode-dependent security posture.
#[cfg(feature = "test-stub-provider")]
#[test]
fn cli_strict_csp_reaches_the_out_dir_bundle() {
    let workdir = ScratchDir::new("transync-cli-csp-outdir");
    let out_dir = workdir.join("published");

    let status = Command::new(bin())
        .arg("translate")
        .arg("--input")
        .arg(fixture_path())
        .arg("--out-dir")
        .arg(&out_dir)
        .arg("--target-language")
        .arg("ko")
        .arg("--strict-csp")
        .status()
        .expect("transync binary must run");
    assert!(status.success(), "exit code: {:?}", status.code());

    let index = std::fs::read_to_string(out_dir.join("html/index.html"))
        .expect("html/index.html should be readable");
    assert!(
        index.contains(CSP_META),
        "--out-dir --strict-csp must harden the html/ bundle too: {index}"
    );
}

/// OI-0018: a run that emits no bundle has nothing to harden, and says so.
/// A security flag that accepts silently is a flag an operator can believe
/// took effect.
#[cfg(feature = "test-stub-provider")]
#[test]
fn cli_strict_csp_without_a_bundle_says_it_did_nothing() {
    let workdir = ScratchDir::new("transync-cli-csp-no-bundle");
    let output = Command::new(bin())
        .arg("translate")
        .arg("--input")
        .arg(fixture_path())
        .arg("--output")
        .arg(workdir.join("out.md"))
        .arg("--map")
        .arg(workdir.join("out.json"))
        .arg("--target-language")
        .arg("ko")
        .arg("--strict-csp")
        .output()
        .expect("transync binary must run");
    assert!(output.status.success(), "exit code: {:?}", output.status);

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--strict-csp has no effect"),
        "the run must name the no-op: {stderr}"
    );
}

/// R0001-0032 helper: a profile TOML carrying one unknown top-level key.
/// `load_profile` ignores the key and records a load warning, which it also
/// emits on the `transync::profile` tracing target — a record that reached a
/// no-op sink until the CLI installed a subscriber.
#[cfg(feature = "test-stub-provider")]
fn profile_with_unknown_key(workdir: &std::path::Path) -> std::path::PathBuf {
    let path = workdir.join("unknown-key.toml");
    std::fs::write(
        &path,
        b"slug = \"probe\"\nversion = \"1.0.0\"\nbogus_key = 1\n\n[system]\nprompt = \"Translate from {{source_language}} to {{target_language}}.\"\n",
    )
    .expect("profile TOML should be writable");
    path
}

/// R0001-0032 helper: one `translate` run whose environment is scrubbed of
/// `RUST_LOG`, so an operator's shell cannot change what these tests observe.
#[cfg(feature = "test-stub-provider")]
fn run_capturing(prefix: &str, extra: &[&std::ffi::OsStr]) -> std::process::Output {
    let workdir = ScratchDir::new(prefix);
    let mut cmd = Command::new(bin());
    cmd.env_remove("RUST_LOG")
        .arg("translate")
        .arg("--input")
        .arg(fixture_path())
        .arg("--output")
        .arg(workdir.join("out.md"))
        .arg("--map")
        .arg(workdir.join("out.json"))
        .arg("--target-language")
        .arg("ko")
        .arg("--profile")
        .arg(profile_with_unknown_key(&workdir));
    for a in extra {
        cmd.arg(a);
    }
    cmd.output().expect("transync binary must run")
}

/// R0001-0032: at default verbosity the library's `tracing::warn!` records
/// reach stderr. Before the CLI installed a subscriber they went to a no-op
/// sink, so cache degradation, retry backoff, batch-fault routing, reparse
/// cascades and profile load warnings were all invisible in the reference
/// binary.
///
/// The observable is the tracing-formatted line — `WARN <target>: <message>`.
/// Nothing else in the CLI can produce it: the command's own diagnostics all
/// carry the `transync: ` prefix and never a level or a target.
///
/// stdout is asserted empty in the same breath: diagnostics must never
/// contaminate a stream a caller might be piping.
#[cfg(feature = "test-stub-provider")]
#[test]
fn cli_library_warnings_reach_stderr_at_default_verbosity() {
    let output = run_capturing("transync-cli-warn-default", &[]);
    assert!(output.status.success(), "exit code: {:?}", output.status);

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("WARN transync::profile:"),
        "a library warn record must reach stderr with its level and target: {stderr}"
    );
    assert!(
        stderr.contains("unknown profile key `bogus_key` ignored"),
        "and carry the message: {stderr}"
    );
    // Exactly once: the pre-R0001-0032 CLI also echoed `load_warnings` from
    // the returned profile, which after the subscriber landed printed the
    // same sentence twice.
    assert_eq!(
        stderr
            .matches("unknown profile key `bogus_key` ignored")
            .count(),
        1,
        "the loader's finding must be reported once, not once per channel: {stderr}"
    );
    // WARN is a floor, not a firehose: `info` records stay below it.
    assert!(
        !stderr.contains("INFO "),
        "default verbosity is warn and above: {stderr}"
    );
    assert!(
        output.stdout.is_empty(),
        "diagnostics belong on stderr: {:?}",
        String::from_utf8_lossy(&output.stdout)
    );
}

/// R0001-0032: `--quiet` means quiet. The subscriber's floor is `OFF`, so
/// the same run that just printed a warning prints nothing at all — the flag
/// governs the library's channel exactly as it governs the command's own.
#[cfg(feature = "test-stub-provider")]
#[test]
fn cli_quiet_silences_the_library_channel() {
    let output = run_capturing(
        "transync-cli-warn-quiet",
        &[std::ffi::OsStr::new("--quiet")],
    );
    assert!(output.status.success(), "exit code: {:?}", output.status);

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.is_empty(),
        "--quiet must suppress the tracing channel too: {stderr}"
    );
}

/// R0001-0032: `--verbose` lowers the floor to `debug`, which is what makes
/// the `info` records visible. The auto-glossary preflight against a
/// translator that cannot extract is one: the pipeline reports the
/// degradation on `tracing::info!`, and no CLI-rendered line covers the
/// `Unsupported` status outside `--verbose`.
#[cfg(feature = "test-stub-provider")]
#[test]
fn cli_verbose_surfaces_info_records() {
    let output = run_capturing(
        "transync-cli-warn-verbose",
        &[
            std::ffi::OsStr::new("--verbose"),
            std::ffi::OsStr::new("--auto-glossary"),
        ],
    );
    assert!(output.status.success(), "exit code: {:?}", output.status);

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("INFO transync::pipeline:"),
        "--verbose must admit info records: {stderr}"
    );
    assert!(
        stderr.contains("does not support glossary extraction"),
        "and carry the degradation the pipeline reported: {stderr}"
    );
    // The warn floor is still under the debug floor.
    assert!(
        stderr.contains("WARN transync::profile:"),
        "--verbose must not lose the warnings: {stderr}"
    );
}

/// R0001-0032: `RUST_LOG` layers over the flag-derived floor rather than
/// replacing it — a per-target directive widens that target and leaves every
/// other one at `warn`.
#[cfg(feature = "test-stub-provider")]
#[test]
fn cli_rust_log_widens_one_target_without_lowering_the_floor() {
    let workdir = ScratchDir::new("transync-cli-rustlog");
    let output = Command::new(bin())
        .env("RUST_LOG", "transync::pipeline=info")
        .arg("translate")
        .arg("--input")
        .arg(fixture_path())
        .arg("--output")
        .arg(workdir.join("out.md"))
        .arg("--map")
        .arg(workdir.join("out.json"))
        .arg("--target-language")
        .arg("ko")
        .arg("--auto-glossary")
        .arg("--profile")
        .arg(profile_with_unknown_key(&workdir))
        .output()
        .expect("transync binary must run");
    assert!(output.status.success(), "exit code: {:?}", output.status);

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("INFO transync::pipeline:"),
        "the directive must widen its target without --verbose: {stderr}"
    );
    assert!(
        stderr.contains("WARN transync::profile:"),
        "and the untargeted warn floor must survive it: {stderr}"
    );
}

/// R0001-0032: the output-budget preflight names each flagged unit once, on
/// the `transync::pipeline` channel; the CLI adds exactly one line, the one
/// naming the flags that move the ceiling. Regression pin for the 2x-per-unit
/// stderr the two channels produced together before the CLI line collapsed.
#[cfg(feature = "test-stub-provider")]
#[test]
fn cli_output_budget_warnings_are_not_printed_twice() {
    let workdir = ScratchDir::new("transync-cli-budget-dedup");
    let output = Command::new(bin())
        .env_remove("RUST_LOG")
        .arg("translate")
        .arg("--input")
        .arg(fixture_path())
        .arg("--output")
        .arg(workdir.join("out.md"))
        .arg("--map")
        .arg(workdir.join("out.json"))
        .arg("--target-language")
        .arg("ko")
        .arg("--target-output-tokens")
        .arg("100")
        .arg("--output-expansion-factor")
        .arg("5.0")
        .output()
        .expect("transync binary must run");
    assert!(output.status.success(), "exit code: {:?}", output.status);

    let stderr = String::from_utf8_lossy(&output.stderr);
    let per_unit = stderr
        .matches("exceeds the per-batch output ceiling")
        .count();
    let traced = stderr.matches("WARN transync::pipeline:").count();
    assert!(
        per_unit > 1,
        "the fixture must flag several units: {stderr}"
    );
    assert_eq!(
        per_unit, traced,
        "every per-unit sentence must come from the tracing channel and nowhere else: {stderr}"
    );
    assert_eq!(
        stderr
            .matches("unit(s) estimated over the per-batch output ceiling")
            .count(),
        1,
        "the CLI contributes one remediation line regardless of unit count: {stderr}"
    );
    assert!(
        stderr.contains("--output-expansion-factor"),
        "and it names both flags: {stderr}"
    );
}

/// R0001-0020: an unknown `{{placeholder}}` is diagnosed whichever door
/// installed the prompt body. `--system-prompt` / `--system-prompt-file`
/// overwrite `prompt_body` *after* `load_profile` has scanned the profile's
/// own template, so before the fix a typo warned from a profile TOML and
/// reached the model in silence through either flag. The emission now has one
/// home — the translate boundary, on the body the run actually compiles — and
/// the loader records the finding on `load_warnings` without emitting it. So
/// the sentence here is a first print, not a second: exactly once, whichever
/// door installed the body (R0001-0032).
#[cfg(feature = "test-stub-provider")]
#[test]
fn cli_system_prompt_override_is_diagnosed() {
    let workdir = ScratchDir::new("transync-cli-prompt-typo");
    let body = "Translate from {{source_language}} to {{target_lang}}.";
    let prompt_file = workdir.join("prompt.txt");
    std::fs::write(&prompt_file, body).expect("prompt file writable");

    let doors = [
        ("--system-prompt", body.to_string()),
        (
            "--system-prompt-file",
            prompt_file.to_string_lossy().into_owned(),
        ),
    ];
    for (i, (flag, value)) in doors.iter().enumerate() {
        let dir = workdir.join(format!("run{i}"));
        std::fs::create_dir_all(&dir).expect("run dir creatable");

        let output = Command::new(bin())
            // The subscriber's level is flag-derived; a stray RUST_LOG in the
            // developer's environment must not decide this assertion.
            .env_remove("RUST_LOG")
            .arg("translate")
            .arg("--input")
            .arg(fixture_path())
            .arg("--output")
            .arg(dir.join("out.md"))
            .arg("--map")
            .arg(dir.join("out.json"))
            .arg("--target-language")
            .arg("ko")
            .arg(flag)
            .arg(value)
            .output()
            .expect("transync binary must run");

        assert!(
            output.status.success(),
            "{flag} must still translate — the typo is a warning, not an error: {:?}",
            output.status.code()
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(
            stderr
                .matches("unknown template variable `{{target_lang}}`")
                .count(),
            1,
            "{flag} must be diagnosed exactly once: {stderr}"
        );
        assert!(
            stderr.contains("WARN transync::profile:"),
            "the diagnostic arrives on the profile tracing target: {stderr}"
        );
    }
}

/// ti ed8c57 (backstop): the diagnostic describes the body the run sends, so
/// an override that *removes* the typo removes the warning with it.
///
/// The loader used to say the sentence the moment it parsed `[system].prompt`,
/// before `resolve_profile` had installed `--system-prompt` over it. A run
/// whose override replaced a typo'd profile body with a clean one therefore
/// still printed "`{{target_lang}}` … will be sent to the model literally"
/// about a body it was not going to send, and nothing downstream could retract
/// it. Three doors, one sentence each, no stale ones:
/// profile-typo-only → once, profile-typo + clean override → never,
/// profile-typo + the same typo in the override → still once (R0001-0032).
#[cfg(feature = "test-stub-provider")]
#[test]
fn cli_prompt_template_warning_follows_the_effective_body() {
    let workdir = ScratchDir::new("transync-cli-prompt-typo-effective");
    let profile_path = workdir.join("typo.toml");
    std::fs::write(
        &profile_path,
        "slug = \"typo\"\nversion = \"1.0.0\"\n[system]\n\
         prompt = \"Translate from {{source_language}} to {{target_lang}}.\"\n",
    )
    .expect("profile file writable");

    let clean = "Translate from {{source_language}} to {{target_language}}.";
    let typo = "Translate from {{source_language}} to {{target_lang}}.";
    let cases: [(&str, Option<&str>, usize); 3] = [
        ("the profile's own typo, nothing over it", None, 1),
        ("a clean override replacing the typo", Some(clean), 0),
        ("an override carrying the same typo", Some(typo), 1),
    ];

    for (i, (what, override_body, expected)) in cases.iter().enumerate() {
        let dir = workdir.join(format!("run{i}"));
        std::fs::create_dir_all(&dir).expect("run dir creatable");

        let mut cmd = Command::new(bin());
        // The subscriber's level is flag-derived; a stray RUST_LOG in the
        // developer's environment must not decide this assertion.
        cmd.env_remove("RUST_LOG")
            .arg("translate")
            .arg("--input")
            .arg(fixture_path())
            .arg("--output")
            .arg(dir.join("out.md"))
            .arg("--map")
            .arg(dir.join("out.json"))
            .arg("--target-language")
            .arg("ko")
            .arg("--profile")
            .arg(&profile_path);
        if let Some(body) = override_body {
            cmd.arg("--system-prompt").arg(body);
        }
        let output = cmd.output().expect("transync binary must run");

        assert!(
            output.status.success(),
            "{what} must still translate — the typo is a warning, not an error: {:?}",
            output.status.code()
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(
            stderr
                .matches("unknown template variable `{{target_lang}}`")
                .count(),
            *expected,
            "{what}: expected {expected} diagnostic(s): {stderr}"
        );
    }
}

/// R0001-0021 + ti fd5aa8: `auto` is a reserved literal, not a label, and it
/// is the sentinel however it is typed. Surrounding whitespace comes off and a
/// recognized sentinel is canonicalized once, at the argument boundary, so
/// every spelling of the same request is indistinguishable downstream — same
/// compiled prompt, same cache identity, same alignment metadata, same bundle.
///
/// Before R0001-0021 a padded label was substituted into the prompt verbatim
/// ("Translate from  auto  to ko"), keyed its own cache entries, and was
/// stamped on the map. Before ti fd5aa8 `AUTO` compiled the auto-detection
/// prompt like `auto` and rendered the same pane, yet still keyed its own
/// cache entries and stamped `"source_language": "AUTO"` on the map.
///
/// The four published artifacts are compared byte-for-byte rather than only
/// the map, because the label reaches more than one of them: `alignment.json`
/// carries it as metadata, `html/index.html` decides the source pane's `lang`
/// from it, and `out.md` is what the prompt it compiled produced.
#[cfg(feature = "test-stub-provider")]
#[test]
fn cli_sentinel_source_language_is_canonical_however_typed() {
    let workdir = ScratchDir::new("transync-cli-sentinel-auto");
    let mut runs = Vec::new();

    for (i, label) in ["auto", "  auto\t", "AUTO", " Auto "].iter().enumerate() {
        let dir = workdir.join(format!("run{i}"));
        let status = Command::new(bin())
            .arg("translate")
            .arg("--input")
            .arg(fixture_path())
            .arg("--out-dir")
            .arg(&dir)
            .arg("--target-language")
            .arg("ko")
            .arg("--source-language")
            .arg(label)
            .status()
            .expect("transync binary must run");
        assert!(
            status.success(),
            "{label:?} should translate: {:?}",
            status.code()
        );

        let map = std::fs::read_to_string(dir.join("alignment.json")).expect("map written");
        let parsed: serde_json::Value = serde_json::from_str(&map).expect("map is JSON");
        assert_eq!(
            parsed["source_language"], "auto",
            "the map must carry the canonical sentinel, not the spelling: {label:?}"
        );
        runs.push((
            label,
            map,
            std::fs::read_to_string(dir.join("out.md")).expect("markdown written"),
            std::fs::read_to_string(dir.join("html").join("index.html")).expect("bundle written"),
        ));
    }

    let (bare, bare_map, bare_md, bare_index) = &runs[0];
    for (label, map, md, index) in &runs[1..] {
        assert_eq!(
            map, bare_map,
            "the alignment map of a {label:?} run must match the {bare:?} one"
        );
        assert_eq!(
            md, bare_md,
            "and so must the translated Markdown: {label:?}"
        );
        assert_eq!(
            index, bare_index,
            "and so must the bundle shell that stamps the source pane's lang: {label:?}"
        );
    }
}

/// R0002-0029: a destination the run was always going to refuse is refused
/// **before the provider is called**, not after.
///
/// The observable is the run's own stderr. `pipeline_diagnostics` lines are
/// emitted between `translate` and the publication, and this fixture always
/// produces one (its verbatim-preserved html block), so their absence is proof
/// that the translation never ran — while their presence in a refused run,
/// which is what the pre-fix binary printed, is proof that a whole paid run
/// was thrown away for a fact the filesystem knew before it started. The
/// refusal itself — exit 4, same sentence — is unchanged.
#[cfg(feature = "test-stub-provider")]
#[test]
fn cli_refuses_a_foreign_html_out_before_translating() {
    let workdir = ScratchDir::new("transync-cli-early-preflight");
    let html_out = workdir.join("html");
    std::fs::create_dir_all(&html_out).unwrap();
    std::fs::write(html_out.join("USER_NOTES.txt"), b"keepme").unwrap();

    let out = Command::new(bin())
        .arg("translate")
        .arg("--input")
        .arg(fixture_path())
        .arg("--output")
        .arg(workdir.join("out.md"))
        .arg("--map")
        .arg(workdir.join("out.json"))
        .arg("--html-out")
        .arg(&html_out)
        .arg("--target-language")
        .arg("ko")
        .output()
        .expect("transync binary must run");
    let stderr = String::from_utf8_lossy(&out.stderr);

    assert_eq!(
        out.status.code(),
        Some(4),
        "a foreign --html-out file is still exit 4: {stderr}"
    );
    assert!(
        stderr.contains("USER_NOTES.txt"),
        "and still names the offending file: {stderr}"
    );
    assert!(
        !stderr.contains("html block"),
        "the pipeline ran before the refusal — the paid call was wasted: {stderr}"
    );
}

/// Review 0004 added three questions to that same early pass, and they are
/// asked on the same terms: the absence of the fixture's `html block` note is
/// the proof that no paid run was thrown away to learn something the arguments
/// and the filesystem already knew.
///
/// - R0004-0067: `--output x --map x/y` names no file twice and still needs
///   `x` to be a file and a directory at once.
/// - R0004-0063: `--force` is consent to destroy foreign content, not
///   permission to skip the question of whether `--html-out` is a directory.
/// - R0004-0066: a destination whose final component is not UTF-8 is one no
///   staging name can be built from — and the old refusal blamed a missing
///   file name the path did not lack.
#[cfg(feature = "test-stub-provider")]
#[test]
fn cli_refuses_the_remaining_destination_faults_before_translating() {
    /// One refused invocation: extra args on top of `--input`/`--target-language`.
    fn refuse(label: &str, extra: &[&std::ffi::OsStr], expected: &str) {
        let out = Command::new(bin())
            .arg("translate")
            .arg("--input")
            .arg(fixture_path())
            .arg("--target-language")
            .arg("ko")
            .args(extra)
            .output()
            .expect("transync binary must run");
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert_eq!(
            out.status.code(),
            Some(4),
            "{label} is a destination refusal, exit 4: {stderr}"
        );
        assert!(
            stderr.contains(expected),
            "{label} must say why ({expected:?}): {stderr}"
        );
        assert!(
            !stderr.contains("html block"),
            "{label} was learned after the paid run: {stderr}"
        );
    }

    let nested = ScratchDir::new("transync-cli-nested-dest");
    let inside = nested.join("x");
    refuse(
        "a destination inside another destination",
        &[
            "--output".as_ref(),
            inside.as_os_str(),
            "--map".as_ref(),
            inside.join("map.json").as_os_str(),
        ],
        "is inside output destination",
    );

    let forced = ScratchDir::new("transync-cli-forced-html-out");
    let bundle = forced.join("bundle");
    std::fs::write(&bundle, b"not a directory").unwrap();
    refuse(
        "a forced --html-out that is a regular file",
        &[
            "--output".as_ref(),
            forced.join("out.md").as_os_str(),
            "--map".as_ref(),
            forced.join("out.json").as_os_str(),
            "--html-out".as_ref(),
            bundle.as_os_str(),
            "--force".as_ref(),
        ],
        "is not a directory",
    );

    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        let raw = ScratchDir::new("transync-cli-non-utf8-dest");
        let bad = raw.join(std::ffi::OsStr::from_bytes(b"out\xffmd"));
        refuse(
            "a destination whose final component is not UTF-8",
            &[
                "--output".as_ref(),
                bad.as_os_str(),
                "--map".as_ref(),
                raw.join("out.json").as_os_str(),
            ],
            "not valid UTF-8",
        );
    }
}

/// D1 §2.4 / test #11: invalid batching-flag values are rejected at clap parse
/// time (range parser for `--max-concurrent-batches`, custom parser for
/// `--output-expansion-factor`), which `main` maps to exit 1.
#[cfg(feature = "test-stub-provider")]
#[test]
fn cli_rejects_invalid_batching_flag_values() {
    for (flag, value) in [
        ("--max-concurrent-batches", "0"),
        ("--output-expansion-factor", "0"),
    ] {
        let workdir = ScratchDir::new("transync-cli-badflag");
        let status = Command::new(bin())
            .arg("translate")
            .arg("--input")
            .arg(fixture_path())
            .arg("--output")
            .arg(workdir.join("out.md"))
            .arg("--map")
            .arg(workdir.join("out.json"))
            .arg("--target-language")
            .arg("ko")
            .arg(flag)
            .arg(value)
            .status()
            .expect("transync binary must run");
        assert_eq!(
            status.code(),
            Some(1),
            "invalid {flag} {value} should exit 1, got {:?}",
            status.code()
        );
    }
}

/// DCR-0028 / OI-0017 — the SCN-10 extension: `--cache-dir` makes the cache
/// outlive the process, and a fully-cache-hit `--source-language auto` run
/// still reports the language the first run's provider detected.
///
/// Two separate `transync` processes share one cache directory. The second is
/// given a provider that would answer a *different* language, so reporting the
/// first run's answer is only possible by replaying the stored document record
/// — a call counter could not distinguish "asked again and got the same value".
/// The bundle comparison is the other half: everything the run publishes is
/// byte-identical across the two, `detected_source_language` included.
#[cfg(feature = "test-stub-provider")]
#[test]
fn cli_cache_dir_persists_progress_and_detected_language() {
    let workdir = ScratchDir::new("transync-cli-cachedir");
    let cache_dir = workdir.join("cache");

    let run = |out_dir: &std::path::Path, detect: &str| {
        let out = Command::new(bin())
            .arg("translate")
            .arg("--input")
            .arg(fixture_path())
            .arg("--out-dir")
            .arg(out_dir)
            .arg("--target-language")
            .arg("ko")
            .arg("--source-language")
            .arg("auto")
            .arg("--cache-dir")
            .arg(&cache_dir)
            .env("TRANSYNC_STUB_DETECT", detect)
            .output()
            .expect("transync binary must run");
        assert!(
            out.status.success(),
            "exit {:?}: {}",
            out.status.code(),
            String::from_utf8_lossy(&out.stderr)
        );
    };

    let first_dir = workdir.join("first");
    run(&first_dir, "en");
    let first_map: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(first_dir.join("alignment.json")).expect("first map written"),
    )
    .expect("first map is JSON");
    assert_eq!(
        first_map["detected_source_language"], "en",
        "the live run reports what its provider detected: {first_map}"
    );
    assert!(
        cache_dir.join("transync-cache.jsonl").is_file(),
        "the run created the log in --cache-dir"
    );

    // Second process, same cache directory, a provider that would say "zz".
    let second_dir = workdir.join("second");
    run(&second_dir, "zz");
    let second_map: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(second_dir.join("alignment.json")).expect("second map written"),
    )
    .expect("second map is JSON");
    assert_eq!(
        second_map["detected_source_language"], "en",
        "the replayed run reports the FIRST run's detection — had it dispatched, \
         its own provider would have said \"zz\": {second_map}"
    );

    // And nothing else moved either: the whole published set matches.
    assert_eq!(
        std::fs::read_to_string(first_dir.join("out.md")).unwrap(),
        std::fs::read_to_string(second_dir.join("out.md")).unwrap(),
        "a cache-hit run must produce byte-identical translated Markdown"
    );
    assert_eq!(
        first_map, second_map,
        "…and a byte-identical alignment map, detected language included"
    );
    assert_eq!(
        std::fs::read_to_string(first_dir.join("html/index.html")).unwrap(),
        std::fs::read_to_string(second_dir.join("html/index.html")).unwrap(),
        "…and a byte-identical bundle"
    );
}

/// ti `dca5bf` — the last provider call a warm run paid: with `--cache-dir`
/// and `--auto-glossary`, a second process makes **zero** calls, the glossary
/// preflight included.
///
/// The two runs are given providers that would harvest *different* renderings
/// of the same term, so the second run's report can only carry the first's
/// harvest by replaying the stored record — a call counter is not observable
/// across a process boundary, and "asked again and got the same answer" would
/// look identical without the differing stubs.
#[cfg(feature = "test-stub-provider")]
#[test]
fn cli_cache_dir_replays_the_auto_glossary_harvest() {
    let workdir = ScratchDir::new("transync-cli-cachedir-glossary");
    let cache_dir = workdir.join("cache");

    let run = |out_dir: &std::path::Path, harvest: &str| {
        let out = Command::new(bin())
            .arg("translate")
            .arg("--input")
            .arg(fixture_path())
            .arg("--out-dir")
            .arg(out_dir)
            .arg("--target-language")
            .arg("ko")
            .arg("--source-language")
            .arg("en")
            .arg("--auto-glossary")
            .arg("--cache-dir")
            .arg(&cache_dir)
            .env("TRANSYNC_STUB_GLOSSARY", harvest)
            .output()
            .expect("transync binary must run");
        assert!(
            out.status.success(),
            "exit {:?}: {}",
            out.status.code(),
            String::from_utf8_lossy(&out.stderr)
        );
    };

    let harvest_of = |dir: &std::path::Path| -> serde_json::Value {
        let report: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(dir.join("validation-report.json"))
                .expect("the report is published"),
        )
        .expect("the report is JSON");
        report["auto_glossary"].clone()
    };

    let first_dir = workdir.join("first");
    run(&first_dir, "tensor=텐서");
    let first = harvest_of(&first_dir);
    assert_eq!(
        first["status"], "extracted",
        "the live run harvested: {first}"
    );
    assert_eq!(first["terms"][0]["target"], "텐서");

    // Second process, same cache directory, a provider that would say "장력".
    let second_dir = workdir.join("second");
    run(&second_dir, "tensor=장력");
    let second = harvest_of(&second_dir);
    assert_eq!(
        second, first,
        "the replayed run reports the FIRST run's harvest — had it called the \
         provider, its own stub would have said \"장력\""
    );

    assert_eq!(
        std::fs::read_to_string(first_dir.join("out.md")).unwrap(),
        std::fs::read_to_string(second_dir.join("out.md")).unwrap(),
        "and the replayed glossary reproduces the unit cache identity, so the \
         document is byte-identical"
    );
}

/// DCR-0028 §6 — an unopenable `--cache-dir` warns once and the run completes
/// on a fresh in-memory cache. At this boundary a cache is an accelerator: an
/// unwritable path must not kill a translation the user asked for.
#[cfg(feature = "test-stub-provider")]
#[test]
fn cli_unopenable_cache_dir_warns_and_completes() {
    let workdir = ScratchDir::new("transync-cli-cachedir-bad");
    // A regular *file* where a directory must be: `create_dir_all` cannot make
    // this into a directory, on any platform.
    let blocked = workdir.join("not-a-dir");
    std::fs::write(&blocked, b"occupied\n").expect("scratch file writable");

    let out = Command::new(bin())
        .arg("translate")
        .arg("--input")
        .arg(fixture_path())
        .arg("--output")
        .arg(workdir.join("out.md"))
        .arg("--map")
        .arg(workdir.join("out.json"))
        .arg("--target-language")
        .arg("ko")
        .arg("--cache-dir")
        .arg(&blocked)
        .output()
        .expect("transync binary must run");
    let stderr = String::from_utf8_lossy(&out.stderr);

    assert!(
        out.status.success(),
        "an unopenable cache must not fail the run: exit {:?}: {stderr}",
        out.status.code()
    );
    assert!(
        stderr.contains("could not open --cache-dir"),
        "the degradation is announced, not silent: {stderr}"
    );
    assert!(
        workdir.join("out.md").is_file(),
        "the run still published its outputs"
    );
}

/// §9: --allow-html-input asserts "Markdown despite the preamble";
/// --input-format html asserts "an HTML document". Together they are an
/// argument error — exit 1, not 2, because the ARGUMENTS are malformed.
/// The bug this catches: clap cannot express a value-dependent conflict,
/// so forgetting the manual check makes the pair silently run.
#[cfg(feature = "test-stub-provider")]
#[test]
fn cli_input_format_conflict_is_an_argument_error() {
    let workdir = ScratchDir::new("transync-cli-fmt-conflict");
    let input = workdir.join("page.html");
    std::fs::write(&input, HTML_DOCUMENT).unwrap();
    let out_md = workdir.join("out.md");
    let out_json = workdir.join("out.json");

    let output = Command::new(bin())
        .arg("translate")
        .arg("--input")
        .arg(&input)
        .arg("--output")
        .arg(&out_md)
        .arg("--map")
        .arg(&out_json)
        .arg("--target-language")
        .arg("ko")
        .arg("--input-format")
        .arg("html")
        .arg("--allow-html-input")
        .output()
        .expect("transync binary must run");

    assert_eq!(
        output.status.code(),
        Some(1),
        "an argument conflict is exit 1"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--allow-html-input") && stderr.contains("--input-format"),
        "the refusal names both flags: {stderr}"
    );
    assert!(
        !out_md.exists() && !out_json.exists(),
        "no output on a refused run"
    );
}

/// §9: `--input-format markdown` alone RE-TRIPS the sniff — the flag names
/// the arm, not a waiver. The bug this catches: keying the sniff on the
/// flag's PRESENCE instead of on the Markdown arm.
#[cfg(feature = "test-stub-provider")]
#[test]
fn cli_explicit_markdown_still_trips_the_sniff() {
    let workdir = ScratchDir::new("transync-cli-explicit-md");
    let input = workdir.join("page.html");
    std::fs::write(&input, HTML_DOCUMENT).unwrap();

    let output = Command::new(bin())
        .arg("translate")
        .arg("--input")
        .arg(&input)
        .arg("--output")
        .arg(workdir.join("out.md"))
        .arg("--map")
        .arg(workdir.join("out.json"))
        .arg("--target-language")
        .arg("ko")
        .arg("--input-format")
        .arg("markdown")
        .output()
        .expect("transync binary must run");
    assert_eq!(
        output.status.code(),
        Some(2),
        "the sniff still refuses, exit 2"
    );
}

/// D8: no reverse sniff. A genuinely-Markdown file declared html produces
/// one text-heavy block set and translates — wrong shape, explicitly
/// requested. The bug this catches: someone "helpfully" adding a
/// Markdown-shape sniff to the html arm.
#[cfg(feature = "test-stub-provider")]
#[test]
fn cli_markdown_declared_html_translates_without_a_reverse_sniff() {
    let workdir = ScratchDir::new("transync-cli-no-reverse-sniff");
    let input = workdir.join("notes.md");
    std::fs::write(&input, "# Title\n\nA paragraph.\n").unwrap();
    let out = workdir.join("out.html");
    let map = workdir.join("out.json");

    let output = Command::new(bin())
        .arg("translate")
        .arg("--input")
        .arg(&input)
        .arg("--input-format")
        .arg("html")
        .arg("--output")
        .arg(&out)
        .arg("--map")
        .arg(&map)
        .arg("--target-language")
        .arg("ko")
        .output()
        .expect("transync binary must run");
    assert_eq!(
        output.status.code(),
        Some(0),
        "explicitly requested is the ADR-0017 boundary: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(out.exists() && map.exists());
}

/// THE operator-visible feature (spec §12 wave 6), end to end over the
/// SCN-16 fixture: out.html (never out.md), the wire's input_format html,
/// the title row's D5 shape, panes that carry anchors and no script, the
/// bundle titled by the source <title> — and out.html ANCHOR-FREE, which
/// is the "anchors never touch the regen path" invariant read off disk.
#[cfg(feature = "test-stub-provider")]
#[test]
fn cli_html_format_run_writes_out_html_and_a_syncing_bundle() {
    let workdir = ScratchDir::new("transync-cli-html-run");
    let input = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../transync/tests/fixtures/scn-16-html-document.html")
        .canonicalize()
        .expect("the SCN-16 fixture exists (wave 3)");
    let out_dir = workdir.join("published");

    let output = Command::new(bin())
        .arg("translate")
        .arg("--input")
        .arg(&input)
        .arg("--input-format")
        .arg("html")
        .arg("--out-dir")
        .arg(&out_dir)
        .arg("--target-language")
        .arg("ko")
        .output()
        .expect("transync binary must run");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(
        out_dir.join("out.html").exists(),
        "an HTML run publishes out.html"
    );
    assert!(
        !out_dir.join("out.md").exists(),
        "one of the two, never both (§9/§6)"
    );

    let published = std::fs::read_to_string(out_dir.join("out.html")).expect("readable");
    assert!(
        published.contains("<!DOCTYPE html>"),
        "gaps are verbatim: doctype"
    );
    assert!(published.contains("<script>"), "gaps are verbatim: script");
    assert!(
        !published.contains("data-sync-id"),
        "out.html is ANCHOR-FREE — injection is a bundle-only derivation \
         and must never reach the regen path (spec §8)"
    );

    let alignment: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(out_dir.join("alignment.json")).expect("map readable"),
    )
    .expect("alignment JSON parses");
    assert_eq!(alignment["schema_version"].as_str(), Some("1.3.0"));
    assert_eq!(alignment["input_format"].as_str(), Some("html"));
    let title = alignment["blocks"]
        .as_array()
        .expect("blocks")
        .iter()
        .find(|b| b["block_kind"].as_str() == Some("title"))
        .expect("the fixture's <title> has a row");
    assert_eq!(title["sync_role"].as_str(), Some("non-sync"), "D5");
    assert_eq!(title["source_format"].as_str(), Some("html"));

    let source_html = std::fs::read_to_string(out_dir.join("html/source.html")).expect("readable");
    let target_html = std::fs::read_to_string(out_dir.join("html/target.html")).expect("readable");
    for pane in [&source_html, &target_html] {
        assert!(
            pane.contains("data-sync-id=\"h1-"),
            "anchors reach the pane"
        );
        assert!(pane.contains("<ul>"), "the nav items share a group (D9)");
        assert!(!pane.contains("<script"), "script is gap, never pane (D6)");
        assert!(
            !pane.contains("data-sync-id=\"title-"),
            "the title has no anchor (D5)"
        );
    }

    // ti 0f26b5's chain, middle rung re-seated for HTML runs (§9): the
    // bundle title is the source document's <title> text — the same
    // extraction the provider saw — entity-decoded then re-escaped by the
    // shell assembler.
    let index = std::fs::read_to_string(out_dir.join("html/index.html")).expect("readable");
    assert!(
        index.contains("<title>Transync &amp; the two-pane page</title>"),
        "flag > source <title> > transync: {index}"
    );
}

/// Deviation 7, both directions: an HTML run's own out-dir republishes —
/// and still republishes after the ownership marker is lost (the ti 66339b
/// cp-drops-dotfiles recovery), which the naive published.len() == 5
/// equality would have silently killed.
#[cfg(feature = "test-stub-provider")]
#[test]
fn cli_html_out_dir_republishes_with_and_without_its_marker() {
    let workdir = ScratchDir::new("transync-cli-html-republish");
    let input = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../transync/tests/fixtures/scn-16-html-document.html")
        .canonicalize()
        .expect("fixture exists");
    let out_dir = workdir.join("published");
    let run = || {
        Command::new(bin())
            .arg("translate")
            .arg("--input")
            .arg(&input)
            .arg("--input-format")
            .arg("html")
            .arg("--out-dir")
            .arg(&out_dir)
            .arg("--target-language")
            .arg("ko")
            .output()
            .expect("transync binary must run")
    };
    assert_eq!(run().status.code(), Some(0), "first publish");
    assert_eq!(run().status.code(), Some(0), "republish over the marker");
    std::fs::remove_file(out_dir.join(".transync-out-dir")).expect("drop the marker");
    let third = run();
    assert_eq!(
        third.status.code(),
        Some(0),
        "the marker-less COMPLETE html set republishes without --force: {}",
        String::from_utf8_lossy(&third.stderr)
    );
}
