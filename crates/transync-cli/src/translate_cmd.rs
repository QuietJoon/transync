//! `transync translate` subcommand.
//!
//! The command is split by concern: [`args`] resolves flags, [`input`] reads
//! files, [`provider`] builds the translator, [`publish`] assembles and
//! commits the output set, and [`report`] renders every human-facing line.
//! This file keeps the clap surface ([`TranslateArgs`]), the flow
//! ([`execute`]), and the exit-code policy that maps each failure to a code
//! ([`CliFailure`]).
//!
//! TRACE: SCN-12
//! TRACE: contracts.md §6

mod args;
mod input;
mod provider;
mod publish;
mod report;

use crate::direction;
use crate::error::ExitCode;
use args::{
    InputFormatArg, apply_batching_overrides, resolve_auto_glossary_flag, resolve_bundle_title,
    resolve_language_and_model_args, resolve_model, resolve_output_target, resolve_title_flag,
};
use clap::Args;
use input::{CappedReadError, ProfileError, html_document_marker, read_capped, resolve_profile};
use provider::translator_for_run;
use publish::{PublishError, preflight_destinations, publish_outputs};
use report::{
    Reporter, diagnostic_line, format_auto_glossary_summary, pipeline_diagnostics, verbose_tally,
};
use std::path::{Path, PathBuf};
use transync::{Cache, DiskCache, InMemoryCache, TranslateOptions, translate_with_cache};

/// Default `--max-input-bytes`: 64 MiB. Files larger than this are refused at
/// the CLI boundary before parsing. EXT-2026-07 P1-7 (ADR-0016 amended).
const DEFAULT_MAX_INPUT_BYTES: u64 = 64 * 1024 * 1024;

/// Argument struct for `transync translate`.
///
/// TRACE: contracts.md §6
#[derive(Debug, Args)]
pub struct TranslateArgs {
    #[arg(long)]
    pub input: PathBuf,
    /// Refuse `--input` files larger than this many bytes before parsing
    /// (admission control; ADR-0016 amended 2026-07-13). The read is capped
    /// via `Read::take(limit + 1)`, so file metadata is never trusted alone.
    /// Default 64 MiB. Does not affect `--profile` / `--system-prompt-file`,
    /// which are capped at a fixed 4 MiB.
    ///
    /// TRACE: EXT-2026-07 P1-7
    #[arg(long = "max-input-bytes", default_value_t = DEFAULT_MAX_INPUT_BYTES)]
    pub max_input_bytes: u64,
    /// Translate an `--input` whose preamble declares an HTML document
    /// (`<!doctype …`, `<html …`) as GFM Markdown anyway, instead of refusing
    /// it with exit 2.
    ///
    /// Without this, such an input is declined at the boundary because the
    /// alternative is not an error: the run exits 0 and names its artifact a
    /// translated document. The runs that open with a tag do translate
    /// correctly, as raw-HTML blocks spliced back byte-exact; everything
    /// between them re-enters as **Markdown**. Three consequences, all
    /// observed rather than inferred (ti `d990b6`): prose is re-read under
    /// Markdown inline rules, so `*`, `_` and `[` are consumed as markup and
    /// entities are decoded; the document title and every section path come
    /// from Markdown headings, so an `<h1>` leaves both empty and the prompt
    /// context degrades with nothing said; and a four-space-indented run
    /// becomes an indented code block, which the engine translates and writes
    /// back **fenced**, so the document that comes out has a shape the one
    /// that went in did not. All three are silent now — the third one used to
    /// be the loud member of the set (it burned three attempts and settled as
    /// `fallback_source`, recorded in the map), and ti `457e51` fixed exactly
    /// that, which removes the only signal this path ever emitted.
    ///
    /// The flag exists because the sniff answers a question about the *first
    /// line* and the operator may know better — a Markdown document that
    /// genuinely opens with a `<html>` island is admissible input, and a guard
    /// with no way past it would be the CLI overruling the library.
    ///
    /// **This is the island case, and only that** (ti `490d97` wave 6). For
    /// an actual HTML document pass `--input-format html`, which routes to a
    /// different intake; `--input-format markdown` alone re-trips the sniff,
    /// so this flag is the only way to say "the Markdown intake despite the
    /// preamble". Passing it together with `--input-format html` is an
    /// argument error (exit 1): the two assert contradictory things about
    /// one input.
    ///
    /// TRACE: ti 13e145
    #[arg(long = "allow-html-input", default_value_t = false)]
    pub allow_html_input: bool,
    /// Which intake parses `--input` (ti `490d97`, D8). `markdown` (the
    /// default) is today's path with the preamble sniff intact — and the
    /// sniff still fires when the flag is passed explicitly, because the
    /// flag names the arm, not a waiver. `html` takes the HTML intake, the
    /// same pipeline and block ids, and writes HTML back out.
    ///
    /// Routing is **flag-only**: there is no reverse sniff on the `html`
    /// arm, because a body fragment is legitimately accepted HTML. A
    /// genuinely-Markdown file declared `html` therefore translates as one
    /// text-heavy block set — wrong shape, but explicitly requested, which
    /// is the boundary ADR-0017's silent-path refusals protect. Conflicts
    /// with `--allow-html-input` (exit 1).
    ///
    /// TRACE: ADR-0025
    #[arg(long = "input-format", value_enum, default_value = "markdown")]
    pub input_format: InputFormatArg,
    /// Translated-document output path — Markdown in, Markdown out; HTML in
    /// (`--input-format html`), HTML out. Required together with `--map`
    /// unless `--out-dir` is given.
    #[arg(long)]
    pub output: Option<PathBuf>,
    /// Alignment-map output path. Required together with `--output` unless
    /// `--out-dir` is given.
    #[arg(long)]
    pub map: Option<PathBuf>,
    /// Publish the complete output set — out.md, alignment.json,
    /// validation-report.json, and the html/ bundle — into one directory via
    /// a staged fileset commit. A fresh target appears with one atomic
    /// rename; replacing an existing target is crash-safe rather than atomic
    /// (the old tree is moved aside first, so a reader looking between the
    /// two renames finds no target). Mutually exclusive with `--output`,
    /// `--map`, and `--html-out`.
    ///
    /// TRACE: EXT-2026-07 P1-6
    /// TRACE: R0001-0035
    #[arg(long = "out-dir", conflicts_with_all = ["output", "map", "html_out"])]
    pub out_dir: Option<PathBuf>,
    #[arg(long = "html-out")]
    pub html_out: Option<PathBuf>,
    /// Add a Content-Security-Policy `<meta>` to the emitted HTML bundle so
    /// the rendered document can load nothing from a remote origin: images,
    /// scripts, styles, frames and connections are confined to the bundle
    /// directory (plus `data:` images). Source Markdown is untrusted
    /// (invariant 7), and a remote image URL in it becomes a tracking pixel
    /// that tells its host when and from where the bundle was opened.
    ///
    /// Off by default, and the default output is byte-identical to what
    /// pre-flag transync emitted: the accepted posture is that source
    /// documents are usually already-local copies whose legitimately-remote
    /// images should keep rendering. Turn it on for documents you did not
    /// write, or bundles you hand to someone else.
    ///
    /// Applies to whichever bundle the run emits (`--html-out` or the
    /// `html/` tree of `--out-dir`). With neither, no bundle exists to
    /// protect and the flag does nothing — the run says so on stderr.
    ///
    /// TRACE: OI-0018
    #[arg(long = "strict-csp", default_value_t = false)]
    pub strict_csp: bool,
    /// Title for the emitted HTML bundle's `<title>` element. Resolution
    /// order: this flag when given, else the source document's first level-1
    /// heading as plain text, else the literal `transync`. An explicitly
    /// empty (or whitespace-only) value is an argument error rather than a
    /// silent fall-through to the other two levels.
    ///
    /// Bundle-only, like `--target-direction`: it never reaches `out.md`, the
    /// alignment map, or the provider.
    #[arg(long = "title")]
    pub title: Option<String>,
    /// Write the per-unit validation report (attempt log + rejection
    /// reasons + provider warnings) as JSON to this path. OI-0012: the
    /// alignment map carries only summary counts; this surfaces the WHY
    /// behind fallbacks without re-running.
    #[arg(long = "validation-report")]
    pub validation_report: Option<PathBuf>,
    /// Target language as an opaque label. BCP-47 codes (`ko`, `ja-JP`,
    /// `zh-Hant`) are the recommended form because they are what the
    /// alignment-map JSON, the cache key, and the system prompt all
    /// surface — but the value is not validated; any non-empty string
    /// is forwarded to the model verbatim.
    #[arg(long = "target-language")]
    pub target_language: String,
    /// Source language hint as an opaque label, or the literal string
    /// `auto` to let the model detect (the detected value comes back
    /// on the alignment map's `detected_source_language` field). Same
    /// "opaque" caveat as `--target-language`, with one exception: `auto`
    /// is reserved, so it is matched case-insensitively and recorded as
    /// `auto` — `AUTO` and `auto` are the same run (ti fd5aa8).
    #[arg(long = "source-language", default_value = "auto")]
    pub source_language: String,
    /// Text direction stamped on the rendered bundle's target pane: `rtl`,
    /// `ltr`, or `auto` (the default). `auto` applies a small built-in RTL
    /// primary-subtag table to the `--target-language` label as a
    /// best-effort presentation hint; the label itself stays opaque
    /// everywhere else (ADR-0013, amended). `ltr` renders left-to-right by
    /// emitting no `dir` attribute at all (the HTML default). Overrides the
    /// active profile's `[render].target_direction`. Affects only the HTML
    /// bundle — never `out.md` or the alignment map.
    ///
    /// TRACE: OI-0032
    #[arg(long = "target-direction", value_enum)]
    pub target_direction: Option<direction::DirectionMode>,
    /// Path to a Profile TOML file. If omitted, the embedded default
    /// profile is used.
    #[arg(long)]
    pub profile: Option<PathBuf>,
    /// Override the active profile's `[system].prompt` template body
    /// with this string. Mutually exclusive with `--system-prompt-file`.
    /// Template variables (`{{source_language}}`, `{{target_language}}`)
    /// are still substituted before each batch.
    #[arg(long = "system-prompt", conflicts_with = "system_prompt_file")]
    pub system_prompt: Option<String>,
    /// Read the system prompt template body from a file. Mutually
    /// exclusive with `--system-prompt`.
    #[arg(long = "system-prompt-file", conflicts_with = "system_prompt")]
    pub system_prompt_file: Option<PathBuf>,
    /// Model ID sent to the provider. Resolution order: this flag when
    /// given, else the `TRANSYNC_OPENAI_MODEL` environment variable,
    /// else `gpt-5-chat-latest` (contracts.md §6).
    #[arg(long)]
    pub model: Option<String>,
    #[arg(long = "base-url")]
    pub base_url: Option<String>,
    /// Directory holding a **disk-backed translation cache** (DCR-0028).
    ///
    /// Absent (the default), each run builds a fresh in-memory cache and
    /// behaves exactly as pre-v0.4.0 transync did. Given, the run opens (and
    /// creates) a `transync-cache.jsonl` log in that directory, so a second run
    /// over the same document under the same profile, model and languages
    /// re-dispatches only what actually changed — and a fully-cache-hit
    /// `--source-language auto` run still reports the language the first run's
    /// provider detected.
    ///
    /// If the directory cannot be opened, the run says so once and continues on
    /// a fresh in-memory cache: at this boundary a cache is an accelerator, and
    /// an unwritable path must not kill a translation the user asked for.
    ///
    /// **One writer at a time.** Two concurrent runs sharing one cache
    /// directory are unsupported; the cost is lost entries (re-translation),
    /// never corrupt output.
    ///
    /// A run killed while the cache was compacting leaves one inert
    /// `transync-cache.jsonl.compact-<pid>-<nanos>` file behind. A later run
    /// that happens to carry the same process id removes it; any other run says
    /// so and leaves it, because another process's id is no proof that it died
    /// — so after a real crash, which restarts under a new id, the file
    /// normally stays until you remove it. Deleting them by hand is safe
    /// whenever no run is active.
    ///
    /// TRACE: DCR-0028
    /// TRACE: SCN-10
    /// TRACE: ti d51ed7
    #[arg(long = "cache-dir")]
    pub cache_dir: Option<PathBuf>,

    /// Run without provider credentials, serving every unit from the cache.
    ///
    /// The disk cache's whole promise is that a document is paid for once
    /// (DCR-0028), and re-rendering a translated document on a machine with no
    /// key — or no network — is where that promise is most obviously wanted.
    /// Until ti `30a744` it could not be kept: the provider was constructed
    /// before the cache was consulted, so a run whose every unit was a hit
    /// still refused to start.
    ///
    /// With this flag the run builds a **credential-free** provider that is
    /// configured identically in every other respect, so it namespaces the
    /// cache byte-for-byte the way the run that warmed it did. A fully warm
    /// run therefore completes with no key at all; a run that misses stops
    /// **at the miss**, with `no provider available for this run` and exit
    /// code 6 — the configuration is what to change, either by dropping this
    /// flag or by warming the cache.
    ///
    /// Requires `--cache-dir`. Without one the run gets a fresh in-memory
    /// cache, so the first unit is guaranteed to miss and the flag could only
    /// ever produce that failure; refusing at argument time says so instead of
    /// spending a parse to arrive there.
    ///
    /// TRACE: ti 30a744
    /// TRACE: OI-0038
    #[arg(long = "offline")]
    pub offline: bool,
    #[arg(long, default_value_t = false)]
    pub force: bool,
    /// Per-batch output-token ceiling: sent to the provider as the response
    /// cap AND used as the budget the output-aware batcher packs against
    /// (D1). Overrides the active profile's `[batching].target_output_tokens`.
    /// `0` disables the ceiling entirely — no cap is sent to the provider and
    /// output-aware packing plus the at-risk preflight are turned off. Any
    /// other value must exceed the 64-token response-envelope reserve; a
    /// smaller one is warned about and ignored, which disables the ceiling too
    /// (R0003-0034).
    #[arg(long = "target-output-tokens")]
    pub target_output_tokens: Option<u32>,
    /// Assumed output-to-source token expansion factor used to estimate each
    /// unit's response size when packing batches; higher yields smaller,
    /// safer batches (D1). Overrides the profile's
    /// `[batching].output_expansion_factor`. Must be a positive, finite
    /// number (values below 1.0 are allowed for targets that compress).
    #[arg(long = "output-expansion-factor", value_parser = args::parse_expansion_factor)]
    pub output_expansion_factor: Option<f64>,
    /// Soft cap on per-batch INPUT tokens. Overrides the built-in default
    /// (6000) and the profile's `[batching].target_input_tokens_per_batch`.
    #[arg(long = "target-input-tokens-per-batch", value_parser = clap::value_parser!(u32).range(1..))]
    pub target_input_tokens_per_batch: Option<u32>,
    /// Hard cap on units per batch regardless of token count. Overrides the
    /// profile's `[batching].max_units_per_batch`.
    #[arg(long = "max-units-per-batch", value_parser = clap::value_parser!(u32).range(1..))]
    pub max_units_per_batch: Option<u32>,
    /// What to do with a table whose estimated response exceeds the output
    /// ceiling (DCR-0026). `row-window-first` (the shipped default) splits it
    /// into header-carrying row windows at packing time and reassembles one
    /// table afterwards; `whole-block` ships it whole, which the preflight
    /// warns about and the provider aborts on. Overrides the active profile's
    /// `[constraints].default_table_strategy`. Inert without a ceiling
    /// (`--target-output-tokens 0` turns both off).
    #[arg(long = "table-strategy", value_parser = ["whole-block", "row-window-first"])]
    pub table_strategy: Option<String>,
    /// Maximum number of in-flight provider requests at once (concurrency).
    /// Overrides the built-in default (6). This is a deployment/runtime knob
    /// and has no profile home.
    #[arg(long = "max-concurrent-batches", value_parser = clap::value_parser!(u32).range(1..))]
    pub max_concurrent_batches: Option<u32>,
    /// Run the auto-glossary extraction preflight (OI-0026): before
    /// batching, an extraction pass harvests recurring source terminology
    /// and merges it into the run's glossary — static profile entries win
    /// on conflict — so every batch pins the same target renderings.
    ///
    /// Off unless this flag or the profile's `auto_glossary = true` asks
    /// for it. The harvest is cached like any other provider answer and is
    /// looked up *before* the call, so it costs one provider call the first
    /// time a given document, static glossary, language pair, model and
    /// provider are seen together, and none on a repeat run, which replays
    /// the hit (ti `dca5bf`; across processes with `--cache-dir`). What
    /// every enabled run does pay is a glossary section on every batch's
    /// system prompt, and a nondeterministic extractor lowers warm-cache
    /// hit rates; the payoff starts at multi-batch documents.
    #[arg(
        long = "auto-glossary",
        default_value_t = false,
        conflicts_with = "no_auto_glossary"
    )]
    pub auto_glossary: bool,
    /// Explicitly disable the auto-glossary preflight, overriding a profile
    /// that sets `auto_glossary = true`.
    #[arg(long = "no-auto-glossary", default_value_t = false)]
    pub no_auto_glossary: bool,
    /// Suppress stderr diagnostics. Mutually exclusive with `--verbose`.
    #[arg(long, default_value_t = false, conflicts_with = "verbose")]
    pub quiet: bool,
    /// Print a one-line validation tally on success. Mutually exclusive
    /// with `--quiet`.
    #[arg(long, default_value_t = false, conflicts_with = "quiet")]
    pub verbose: bool,
}

/// A run that ended before producing its outputs, carrying the exit code the
/// process must surface and the one line explaining why.
///
/// TRACE: contracts.md §6
pub(crate) struct CliFailure {
    pub(crate) code: ExitCode,
    pub(crate) message: String,
}

impl CliFailure {
    fn new(code: ExitCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

/// What a successful run leaves for the caller to print: the validation
/// tallies and the auto-glossary outcome, both `--verbose`-only.
pub(crate) struct RunSummary {
    validation: transync::ValidationSummary,
    auto_glossary: Option<transync::AutoGlossaryReport>,
}

/// Execute the subcommand. Reads `--input`, runs the pipeline, atomically
/// writes the four output paths, and surfaces exit codes 0..7 per
/// `contracts.md` §6.
///
/// TRACE: SCN-12
pub async fn run(args: TranslateArgs) -> i32 {
    let reporter = Reporter::new(args.quiet);
    let summary = match execute(&args, &reporter).await {
        Ok(s) => s,
        Err(failure) => {
            reporter.say(&failure.message);
            return failure.code as i32;
        }
    };

    if args.verbose {
        // `--verbose` and `--quiet` conflict at parse time, so these lines
        // are never subject to the quiet gate — but they are still rendered
        // through the same one-line boundary as every other `transync: ` line
        // (R0004-0071): the auto-glossary summary embeds the provider's own
        // failure text, which is not this program's prose.
        eprintln!("{}", diagnostic_line(&verbose_tally(&summary.validation)));
        // OI-0026: one line naming what the preflight contributed. Absent
        // entirely when the feature was off.
        if let Some(ag) = &summary.auto_glossary {
            eprintln!("{}", diagnostic_line(&format_auto_glossary_summary(ag)));
        }
    }

    ExitCode::Success as i32
}

/// The exit code and the diagnostic for one publication failure.
///
/// One function because the same failures can now arrive from two places: the
/// early destination preflight (before the provider call, R0002-0029) and the
/// publication itself. An operator must not be able to tell which pass refused
/// by reading the message.
fn publish_failure(e: PublishError) -> CliFailure {
    match e {
        PublishError::SerializeAlignment(e) => CliFailure::new(
            ExitCode::Other,
            format!("could not serialize alignment map: {e}"),
        ),
        PublishError::SerializeReport(e) => CliFailure::new(
            ExitCode::Other,
            format!("could not serialize validation report: {e}"),
        ),
        PublishError::HtmlOutPreflight { dir, source } => CliFailure::new(
            ExitCode::WriteFailure,
            format!(
                "could not write --html-out bundle to {}: {source}",
                dir.display()
            ),
        ),
        PublishError::Commit(e) => CliFailure::new(
            ExitCode::WriteFailure,
            format!("could not write outputs: {e}"),
        ),
    }
}

/// The command's flow. Every failure leaves through [`CliFailure`], so the
/// mapping from failure to exit code and diagnostic is this function's
/// `Err` arms and nowhere else (`contracts.md` §6). The one arm that cannot
/// answer with a literal — the pipeline's own failure, which carries the
/// provider taxonomy — delegates to [`ExitCode::for_pipeline_failure`], the
/// single table for that classification.
async fn execute(args: &TranslateArgs, reporter: &Reporter) -> Result<RunSummary, CliFailure> {
    // Resolve the output destination first: an argument-level mismatch
    // (missing --output/--map with no --out-dir) should fail before any file
    // is read or the provider is called. EXT-2026-07 P1-6.
    let output_target =
        resolve_output_target(args).map_err(|msg| CliFailure::new(ExitCode::ArgumentError, msg))?;

    // ti 0f26b5: the same "argument-level mistakes fail before any file is
    // read" reasoning. `--title ''` is a usage error, not a request for the
    // fallback, and finding that out after a paid translation run would be
    // the expensive way to learn it.
    let title_flag =
        resolve_title_flag(args).map_err(|msg| CliFailure::new(ExitCode::ArgumentError, msg))?;

    // ti 490d97 wave 6 (§9): the pair asserts contradictory things about one
    // input — "Markdown despite its preamble" / "an HTML document". Exit 1,
    // not 2: the ARGUMENTS are malformed (§6's code-1/code-2 boundary). Clap
    // cannot express a value-dependent conflict, so it is enforced here.
    // Exhaustive by charter.
    match args.input_format {
        InputFormatArg::Html => {
            if args.allow_html_input {
                return Err(CliFailure::new(
                    ExitCode::ArgumentError,
                    "--allow-html-input says the input is Markdown despite its preamble; \
                     --input-format html says it is an HTML document. Pass one or the other",
                ));
            }
        }
        InputFormatArg::Markdown => {}
    }

    // R0011-0076 / R0011-0077 / R0011-0078 / R0011-0079: everything from here
    // to the destination preflight is answered by **argv and the environment
    // alone** — a blank label, a mode combination that can never run, a
    // `--base-url` that is not a URL, an absent credential. Each used to be
    // discovered after the input and the profile had been read and decoded, so
    // a run with two faults reported the parse failure and left the spelling
    // mistake for the next attempt. This is the reasoning
    // `resolve_output_target` and `resolve_title_flag` above already follow,
    // applied to the rest of the argument surface.
    //
    // R0008-0039: validate all three identifiers consistently before they
    // travel into the pipeline / cache key / provider request.
    // R0001-0021: the same call normalizes the two labels, and `languages` is
    // what every consumer below reads — nothing downstream goes back to
    // `args.source_language` / `args.target_language`, so the padding cannot
    // reach the prompt through one path while another has already dropped it.
    let languages = resolve_language_and_model_args(args)
        .map_err(|msg| CliFailure::new(ExitCode::ArgumentError, msg))?;

    // ti `30a744`: `--offline` is only coherent against a cache that outlives
    // the process. A fresh in-memory cache misses its first lookup by
    // construction, so the flag would have exactly one possible outcome —
    // refuse here, where the message can say which flag to add, rather than
    // after a parse and a batch-packing pass.
    if args.offline && args.cache_dir.is_none() {
        return Err(CliFailure::new(
            ExitCode::ArgumentError,
            "--offline needs --cache-dir: without a cache that outlives the run \
             every unit misses, and an offline run that misses cannot proceed"
                .to_string(),
        ));
    }

    let model = resolve_model(args.model.as_deref());

    // Configuration only: the adapter is built and its arguments validated, and
    // no request is issued until the pipeline runs it far below.
    let translator = translator_for_run(&model, args.base_url.as_deref(), args.offline)
        .map_err(|msg| CliFailure::new(ExitCode::ArgumentError, msg))?;

    // R0002-0029: the destination guards need no translated content, so they
    // run before the provider does. A foreign --html-out directory or an
    // --out-dir target that is not a prior out-dir is a refusal the filesystem
    // could already have given before the run — collecting it after the paid
    // translation discards the whole run for nothing. The authoritative pass
    // still runs inside publish_outputs (under the publication lock for
    // --out-dir), so this is an early answer, not a replacement.
    preflight_destinations(&output_target, args).map_err(publish_failure)?;

    // EXT-2026-07 P1-7: admission-control the input read. read_capped reads
    // limit+1 bytes so an oversized file is rejected (exit 2) instead of read
    // whole into memory.
    let source = read_capped(&args.input, args.max_input_bytes).map_err(|e| match e {
        CappedReadError::TooLarge { limit } => CliFailure::new(
            ExitCode::InputReadFailure,
            format!(
                "--input {} exceeds the --max-input-bytes limit of {limit} bytes",
                args.input.display()
            ),
        ),
        CappedReadError::Io(e) => CliFailure::new(
            ExitCode::InputReadFailure,
            format!("could not read --input {}: {e}", args.input.display()),
        ),
        CappedReadError::NotUtf8 => CliFailure::new(
            ExitCode::InputReadFailure,
            format!("--input {} is not valid UTF-8", args.input.display()),
        ),
    })?;

    // ti 13e145: admission control on the input's FORMAT, in the same place
    // and on the same exit code as the size and UTF-8 checks above — an
    // `--input` this command cannot translate is an input failure, which is
    // what code 2 already means (contracts.md §6, and the arm that maps
    // `TransyncError::Parse` there). It is deliberately NOT code 1: the
    // arguments are well-formed and the file is readable; what is wrong is the
    // document, and a script that fixes its own argv and retries must not be
    // told those are the same event.
    // ti 490d97 wave 6 (D8): the sniff is consulted on the MARKDOWN arm only.
    // The Html arm is empty — the sniff is not consulted, and there is no
    // reverse sniff, because a body fragment is legitimately accepted HTML.
    // Exhaustive by charter.
    match args.input_format {
        InputFormatArg::Html => {}
        InputFormatArg::Markdown => {
            if !args.allow_html_input
                && let Some(marker) = html_document_marker(&source)
            {
                return Err(CliFailure::new(
                    ExitCode::InputReadFailure,
                    format!(
                        "--input {} looks like an HTML document (its preamble opens with `{marker}`) and \
                 transync translates GFM Markdown. Translated as Markdown it would not fail — it \
                 would exit 0 over output nothing reports as wrong: text between the tag-opening \
                 runs re-enters as Markdown, so its prose is re-read under Markdown inline rules \
                 (`*`, `_` and `[` become markup, entities are decoded); the document title and \
                 the section context come from Markdown headings, so an <h1> leaves both empty \
                 and the prompt degrades silently; and a four-space-indented run becomes a code \
                 block that is translated and written back FENCED, so the document changes shape \
                 and nothing reports it. Pass --input-format html to translate it as an HTML \
                 document, or --allow-html-input to translate it as Markdown anyway (ti 490d97)",
                        args.input.display()
                    ),
                ));
            }
        }
    }

    let mut profile = resolve_profile(
        args.profile.as_deref(),
        args.system_prompt.as_deref(),
        args.system_prompt_file.as_deref(),
    )
    .map_err(|e| match e {
        ProfileError::Arg(msg) => CliFailure::new(ExitCode::ArgumentError, msg),
        ProfileError::ReadTooLarge(msg) => CliFailure::new(ExitCode::InputReadFailure, msg),
    })?;
    // contracts.md §2 (OI-0003): the loader's findings surface on stderr.
    // R0001-0032: they get there through the `transync::profile` tracing
    // target now, and `logging::init` gives that channel a destination.
    // Re-printing them from `load_warnings` here emitted each one twice,
    // differing only in prefix. `--quiet` still silences them (the
    // subscriber's floor is OFF), so the flag contract is unchanged.
    //
    // `load_warnings` is the record of what the *file* said and is a superset
    // of what was emitted at load time: two of its entries are claims about
    // what a later stage compiles, so the stage that knows the effective text
    // raises them instead — the unknown-`{{placeholder}}` scan at the
    // translate boundary (ti ed8c57) and the glossary control-character
    // advisory at the batching door (ti 5f6664). Echoing the field here would
    // print those a second time.

    // D1: overlay the CLI batching flags onto the resolved profile
    // (flag > profile > built-in default) so there is exactly one downstream
    // resolution mechanism. Same spot the `--system-prompt` mutation already
    // uses (inside resolve_profile just above). Must run before `args`
    // fields are moved into `opts`.
    apply_batching_overrides(&mut profile, args);

    // OI-0032: the profile moves into `opts` below, so capture the
    // presentation-only direction hint (consumed at bundle-assembly time,
    // well after the move) while it is still reachable.
    let profile_target_direction = profile.render.target_direction.clone();

    let mut opts = TranslateOptions::default();
    opts.source_language = languages.source.clone();
    opts.target_language = languages.target.clone();
    opts.model_id = model.clone();
    opts.profile = Some(profile);
    // D1: `--max-concurrent-batches` is the one batching flag with no profile
    // home (runtime knob), so it writes straight onto `opts`; every other
    // batching flag — including `--target-input-tokens-per-batch` — was
    // overlaid onto the profile above so an explicit flag always wins, even
    // when it equals the built-in default.
    if let Some(v) = args.max_concurrent_batches {
        opts.max_concurrent_batches = v;
    }
    // OI-0026: `--auto-glossary` / `--no-auto-glossary` write onto `opts`
    // rather than the profile, because the pipeline's own resolution rule is
    // flag > profile > built-in `false`, and `Option<bool>` keeps an
    // explicit `false` distinguishable from unset (so `--no-auto-glossary`
    // can override a profile that enables it).
    opts.auto_glossary = resolve_auto_glossary_flag(args);
    opts.input_format = args.input_format.to_source_format();

    // DCR-0028 §6: `--cache-dir` is the whole difference between a throwaway
    // per-run cache and one that outlives the process. Resolved here, right
    // before the run, so an open failure is reported with the run's other
    // diagnostics rather than during argument parsing.
    let cache = resolve_cache(args.cache_dir.as_deref(), reporter);

    let output = translate_with_cache(&source, &opts, translator.as_ref(), cache.as_ref())
        .await
        .map_err(|e| {
            // contracts.md §6. The one arm of this function with more than
            // one possible answer, so the classification lives beside the
            // enum (`ExitCode::for_pipeline_failure`) where the whole table
            // is readable at once — parse failures to 2, the configuration
            // and document halves of the provider taxonomy to 6 and 7
            // (ti `e62b59`), everything else to 5.
            CliFailure::new(
                ExitCode::for_pipeline_failure(&e),
                format!("translation failed: {e}"),
            )
        })?;

    for line in pipeline_diagnostics(&output.validation_report, &output.alignment_map) {
        reporter.say(&line);
    }

    // ti 0f26b5: flag > the document's own first H1 > the "transync" literal.
    // Resolved here because the middle level only exists once the pipeline has
    // parsed the document; the flag half was validated before the run started.
    let bundle_title =
        resolve_bundle_title(title_flag.as_deref(), output.document_title.as_deref());

    publish_outputs(
        &output_target,
        args,
        &languages,
        &bundle_title,
        &output,
        profile_target_direction.as_deref(),
        reporter,
    )
    .map_err(publish_failure)?;

    // Exit 3 — every translatable unit fell back to source. Outputs are
    // still written so downstream consumers can inspect them.
    let validation = output.alignment_map.validation_summary.clone();
    if validation.total_units > 0 && validation.fallback_source == validation.total_units {
        return Err(CliFailure::new(
            ExitCode::AllUnitsFellBack,
            format!(
                "every translatable unit fell back to source ({}/{})",
                validation.fallback_source, validation.total_units,
            ),
        ));
    }

    Ok(RunSummary {
        validation,
        auto_glossary: output.validation_report.auto_glossary.clone(),
    })
}

/// Resolve the run's `Cache` from `--cache-dir` (DCR-0028 §6).
///
/// With no flag the run gets a fresh [`InMemoryCache`] — byte-identical
/// behavior to pre-v0.4.0 transync, which built exactly that inside
/// `translate`. With one, the run opens a [`DiskCache`] there.
///
/// An open failure **warns once and degrades** rather than aborting: at the CLI
/// boundary a cache is an accelerator, and an unwritable directory must not
/// kill a translation the user asked for. This is the one place the
/// construction error is handled — every mid-run cache error is already the
/// pipeline's warn-and-degrade policy, which a `CacheError` can never escape.
///
/// TRACE: DCR-0028
/// TRACE: contracts.md §6
fn resolve_cache(cache_dir: Option<&Path>, reporter: &Reporter) -> Box<dyn Cache> {
    let Some(dir) = cache_dir else {
        return Box::new(InMemoryCache::new());
    };
    match DiskCache::open(dir) {
        Ok(cache) => Box::new(cache),
        Err(e) => {
            reporter.say(&format!(
                "could not open --cache-dir {}: {e}; continuing with a fresh in-memory cache \
                 (this run will not reuse or persist anything)",
                dir.display()
            ));
            Box::new(InMemoryCache::new())
        }
    }
}
