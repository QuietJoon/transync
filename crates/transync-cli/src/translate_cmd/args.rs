//! Flag resolution for `transync translate`: pure functions turning the
//! parsed [`TranslateArgs`] into the values the run needs (output
//! destination, model id, profile overlay, tri-state opt-ins) plus the
//! clap value-parsers. Nothing here reads or writes files.
//!
//! TRACE: contracts.md §6

use super::TranslateArgs;
use std::path::PathBuf;
use transync::profile::ProfileMetadata;

/// The run's two language labels, normalized once — this is the only place
/// the CLI touches them, and every consumer downstream reads these fields
/// rather than the raw `TranslateArgs` (R0001-0021).
///
/// ADR-0013 keeps a *label* opaque: no BCP-47 parsing, no case folding,
/// nothing removed from its interior. What is normalized here is never label
/// content, and both exceptions exist for one reason — the value reaches the
/// compiled system prompt, the user-message payload, the cache key, the
/// alignment map and the bundle's `lang` attribute, so anything the sentinel
/// test ignores while the stored label keeps it makes one run answer
/// differently in different places:
///
/// - **Surrounding whitespace** comes off both labels (R0001-0021): ` auto `
///   used to key its own cache identity, stamp its own alignment metadata,
///   and compile a prompt naming a literal padded label instead of the
///   auto-detection phrase.
/// - **A recognized sentinel is stored in its canonical spelling** (ti
///   fd5aa8). The sentinel test is case-insensitive, so `AUTO` already
///   compiled the auto-detection prompt and already rendered the source pane
///   from the *detected* label — while the cache key and the alignment map
///   carried the literal `AUTO`: a different cache identity and different
///   metadata for a request byte-identical to `auto`'s. Only the reserved
///   literal is folded; every other label keeps its case byte-for-byte.
pub(crate) struct LanguageLabels {
    /// `--source-language`, trimmed, and spelled [`SOURCE_LANGUAGE_SENTINEL`]
    /// whenever [`is_source_language_sentinel`] accepts it — so a consumer can
    /// compare it either way and get the same answer.
    pub(crate) source: String,
    /// `--target-language`, trimmed. Nothing is folded here: no literal is
    /// reserved on this side, so `auto` is an ordinary (if unhelpful) target
    /// label and `AUTO` is a different one.
    pub(crate) target: String,
}

/// The one reserved source-language literal, in its canonical spelling:
/// `--source-language auto` asks the model to detect the language instead of
/// naming one, and compiles to *"the auto-detected source language"* rather
/// than being substituted (ADR-0013).
pub(crate) const SOURCE_LANGUAGE_SENTINEL: &str = "auto";

/// Does this *already trimmed* label ask for source-language detection?
///
/// The CLI's single sentinel recognizer (ti fd5aa8):
/// [`resolve_language_and_model_args`] canonicalizes with it and
/// `publish::pane_source_language` asks it, so no two consumers can disagree
/// about whether a run requested detection. Case-insensitive, matching the
/// library's own test in `profile::render_prompt_body`; padding is not
/// considered here because the argument boundary has already removed it.
pub(crate) fn is_source_language_sentinel(label: &str) -> bool {
    label.eq_ignore_ascii_case(SOURCE_LANGUAGE_SENTINEL)
}

/// Reject blank language/model identifiers before they reach the pipeline,
/// cache key, or provider request (R0008-0039), and hand back the normalized
/// labels the rest of the run uses. `auto` is the documented sentinel for
/// source-language detection; a blank value is an error, and an explicit empty
/// `--model` must not silently fall through to the default.
///
/// R0001-0021: validating a trimmed value and then forwarding the untrimmed
/// one was the split that let padding through, so the check and the
/// normalization are now the same act. ti fd5aa8 closes the same split for
/// the sentinel's spelling.
pub(crate) fn resolve_language_and_model_args(
    args: &TranslateArgs,
) -> Result<LanguageLabels, &'static str> {
    let target = args.target_language.trim();
    if target.is_empty() {
        return Err("--target-language must not be empty");
    }
    let source = args.source_language.trim();
    if source.is_empty() {
        return Err("--source-language must not be empty (use \"auto\" to detect)");
    }
    // The model id is validated here but not normalized: it is a provider
    // identifier, not a label this command owns, and `resolve_model` has its
    // own env/default resolution to answer for.
    if args.model.as_deref().is_some_and(|m| m.trim().is_empty()) {
        return Err("--model must not be empty");
    }
    Ok(LanguageLabels {
        // ti fd5aa8: the sentinel is a literal this command reserves, not a
        // label the caller owns, so a recognized one is stored canonically.
        // `AUTO` and `auto` then name one run everywhere — same compiled
        // prompt, same wire payload, same cache identity, same alignment
        // metadata. A value the recognizer rejects is passed through
        // untouched, case included (ADR-0013).
        source: if is_source_language_sentinel(source) {
            SOURCE_LANGUAGE_SENTINEL.to_string()
        } else {
            source.to_string()
        },
        target: target.to_string(),
    })
}

/// What the bundle's `<title>` says when neither `--title` nor a source
/// document H1 supplies one. It is the crate name, not a document name — the
/// third precedence level exists so the shell always has *a* title, and this
/// is the value pre-ti-0f26b5 bundles carried unconditionally.
///
/// TRACE: contracts.md §6
pub(crate) const FALLBACK_BUNDLE_TITLE: &str = "transync";

/// Validate `--title` at the argument boundary and hand back the normalized
/// value: `None` when the flag is absent, `Some(trimmed)` otherwise.
///
/// A given-but-blank title is an argument error (exit 1) for the reason an
/// empty `--model` is one: the two other precedence levels below it are
/// *fallbacks*, not a place to quietly send a flag the user did explicitly
/// pass. Running it here — before the input read and before any provider call
/// — means the mistake costs nothing.
///
/// The trim is the same normalization the language labels get (R0001-0021):
/// surrounding whitespace is not part of any title anyone means to write, and
/// it is not inert in `<title>` either.
///
/// TRACE: contracts.md §6
pub(crate) fn resolve_title_flag(args: &TranslateArgs) -> Result<Option<String>, &'static str> {
    match args.title.as_deref() {
        None => Ok(None),
        Some(t) if t.trim().is_empty() => Err("--title must not be empty"),
        Some(t) => Ok(Some(t.trim().to_string())),
    }
}

/// Resolve the bundle's `<title>` text: **flag > first H1 > `transync`**
/// (owner decision 2026-08-06, ti 0f26b5).
///
/// `flag` is [`resolve_title_flag`]'s output — already trimmed and known
/// non-blank. `document_title` is `TranslationOutput::document_title`: the
/// plain text of the source document's first level-1 heading, extracted by the
/// same code that told the provider what the document is called. It is
/// untrusted content (invariant 7) and may be empty or whitespace-only (`#`
/// with no text), which is not a title — so a blank one falls through to the
/// literal rather than emitting an empty `<title>`.
///
/// Escaping is the assembler's job, not this function's: the value returned
/// here is text, and `crate::output::html_bundle_files` escapes it on the way
/// into the shell.
///
/// TRACE: contracts.md §6
pub(crate) fn resolve_bundle_title(flag: Option<&str>, document_title: Option<&str>) -> String {
    flag.or_else(|| document_title.map(str::trim).filter(|t| !t.is_empty()))
        .unwrap_or(FALLBACK_BUNDLE_TITLE)
        .to_string()
}

/// Resolve the model ID: an explicit `--model` wins; the
/// `TRANSYNC_OPENAI_MODEL` environment variable applies when the flag
/// is absent; the documented default otherwise. Mirrors the
/// `--base-url`/`TRANSYNC_OPENAI_BASE_URL` resolution.
///
/// TRACE: contracts.md §6
pub(crate) fn resolve_model(flag: Option<&str>) -> String {
    flag.map(str::to_string)
        .or_else(|| {
            std::env::var("TRANSYNC_OPENAI_MODEL")
                .ok()
                .filter(|v| !v.trim().is_empty())
        })
        .unwrap_or_else(|| "gpt-5-chat-latest".to_string())
}

/// Clap value-parser for `--output-expansion-factor`: accept only a finite,
/// strictly-positive `f64`. Rejecting at parse time turns a bad value into a
/// clap usage error (which `main` maps to exit 1) rather than a silently
/// ignored flag. D1 §2.4.
pub(crate) fn parse_expansion_factor(s: &str) -> Result<f64, String> {
    let f: f64 = s.parse().map_err(|_| format!("`{s}` is not a number"))?;
    if !f.is_finite() || f <= 0.0 {
        return Err(format!(
            "expansion factor must be a positive, finite number, got `{s}`"
        ));
    }
    Ok(f)
}

/// Overlay the CLI batching flags onto the resolved profile so a single
/// downstream mechanism resolves them (flag > profile > built-in default).
/// `--target-output-tokens 0` is the documented "no ceiling" sentinel and
/// maps to `None` (the value that omits the cap from the provider request and
/// disables output-aware packing + the preflight).
/// `--target-input-tokens-per-batch` overlays too: writing it onto
/// `TranslateOptions` instead would make an explicit flag equal to the
/// built-in default (6000) indistinguishable from unset under
/// `build_batches`'s caller-non-default rule, letting a profile value
/// silently beat the user's explicit flag. Only `--max-concurrent-batches`
/// (a runtime knob with no profile home) is written directly onto
/// `TranslateOptions` by the caller. D1 §2.4.
pub(crate) fn apply_batching_overrides(profile: &mut ProfileMetadata, args: &TranslateArgs) {
    if let Some(v) = args.target_output_tokens {
        profile.batching.target_output_tokens = (v > 0).then_some(v);
    }
    if let Some(f) = args.output_expansion_factor {
        profile.batching.output_expansion_factor = Some(f);
    }
    if let Some(v) = args.max_units_per_batch {
        profile.batching.max_units_per_batch = Some(v);
    }
    if let Some(v) = args.target_input_tokens_per_batch {
        profile.batching.target_input_tokens_per_batch = Some(v);
    }
    // DCR-0026: `--table-strategy` is a `[constraints]` key rather than a
    // `[batching]` one, but it is overlaid here for the same reason and under
    // the same rule — flag > profile > built-in default — so one place
    // answers "what did the command line change about the profile". Clap has
    // already rejected any value that is not one of the two.
    if let Some(v) = &args.table_strategy {
        profile.constraints.default_table_strategy = Some(v.clone());
    }
}

/// Resolve the auto-glossary opt-in/opt-out pair into the tri-state the
/// pipeline expects. `--auto-glossary` → `Some(true)`,
/// `--no-auto-glossary` → `Some(false)` (which beats a profile that enables
/// it), neither → `None` (defer to the profile, else off). Clap rejects
/// both at parse time, so the ordering below is not a precedence policy.
///
/// TRACE: OI-0026
pub(crate) fn resolve_auto_glossary_flag(args: &TranslateArgs) -> Option<bool> {
    if args.auto_glossary {
        Some(true)
    } else if args.no_auto_glossary {
        Some(false)
    } else {
        None
    }
}

/// The resolved output destination. Either individual `--output`/`--map`
/// paths (plus the optional `--html-out` bundle and `--validation-report`)
/// or a single `--out-dir` directory published atomically.
///
/// TRACE: EXT-2026-07 P1-6
pub(crate) enum OutputTarget {
    Files { output: PathBuf, map: PathBuf },
    Dir { dir: PathBuf },
}

/// Resolve which output mode the flags select. `--out-dir` is mutually
/// exclusive with `--output`/`--map`/`--html-out` (enforced by clap at parse
/// time, so an `--out-dir` here means the others are absent); when it is not
/// given, both `--output` and `--map` are required.
///
/// TRACE: EXT-2026-07 P1-6
pub(crate) fn resolve_output_target(args: &TranslateArgs) -> Result<OutputTarget, String> {
    if let Some(dir) = &args.out_dir {
        return Ok(OutputTarget::Dir { dir: dir.clone() });
    }
    match (&args.output, &args.map) {
        (Some(output), Some(map)) => Ok(OutputTarget::Files {
            output: output.clone(),
            map: map.clone(),
        }),
        (None, None) => {
            Err("one of --out-dir, or both --output and --map, is required".to_string())
        }
        (Some(_), None) => Err("--output requires --map (or use --out-dir)".to_string()),
        (None, Some(_)) => Err("--map requires --output (or use --out-dir)".to_string()),
    }
}

// D1 §2.4: `apply_batching_overrides` precedence + the `--output-expansion-factor`
// value-parser. Constructing `TranslateArgs` via a small clap harness keeps the
// test robust to future field additions (defaults fill in).
/// Which intake parses `--input` (D8: routing is flag-only; the sniff stays
/// a refusal, never a router).
///
/// TRACE: ti 490d97 wave 6 (spec 2026-08-20 §9)
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub(crate) enum InputFormatArg {
    Markdown,
    Html,
}

impl InputFormatArg {
    /// The library-side format this flag names. Exhaustive by charter: a
    /// third intake must stop the compiler here.
    pub(crate) fn to_source_format(self) -> transync::SourceFormat {
        match self {
            InputFormatArg::Markdown => transync::SourceFormat::Markdown,
            InputFormatArg::Html => transync::SourceFormat::Html,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use transync::TranslateOptions;
    use transync::profile::default_profile;

    #[derive(Parser)]
    struct Harness {
        #[command(flatten)]
        args: TranslateArgs,
    }

    fn parse_args(extra: &[&str]) -> TranslateArgs {
        parse_args_with_target("ko", extra)
    }

    /// As [`parse_args`], but the caller chooses the target label: the fixed
    /// argv already carries one and clap refuses a second occurrence.
    fn parse_args_with_target(target: &str, extra: &[&str]) -> TranslateArgs {
        let mut argv = vec!["transync", "--input", "in.md", "--target-language", target];
        argv.extend_from_slice(extra);
        Harness::try_parse_from(argv)
            .expect("args should parse")
            .args
    }

    #[test]
    fn overlay_flag_beats_profile_value() {
        let mut profile = default_profile();
        profile.batching.target_output_tokens = Some(8000);
        profile.batching.output_expansion_factor = Some(2.0);
        profile.batching.max_units_per_batch = Some(8);

        let args = parse_args(&[
            "--target-output-tokens",
            "1234",
            "--output-expansion-factor",
            "1.3",
            "--max-units-per-batch",
            "4",
        ]);
        apply_batching_overrides(&mut profile, &args);
        assert_eq!(profile.batching.target_output_tokens, Some(1234));
        assert_eq!(profile.batching.output_expansion_factor, Some(1.3));
        assert_eq!(profile.batching.max_units_per_batch, Some(4));
    }

    #[test]
    fn zero_target_output_tokens_disables_ceiling() {
        let mut profile = default_profile();
        profile.batching.target_output_tokens = Some(8000);
        let args = parse_args(&["--target-output-tokens", "0"]);
        apply_batching_overrides(&mut profile, &args);
        assert_eq!(
            profile.batching.target_output_tokens, None,
            "the 0 sentinel disables the ceiling even over an explicit profile value"
        );
    }

    #[test]
    fn absent_flags_preserve_profile_values() {
        let mut profile = default_profile();
        profile.batching.target_output_tokens = Some(7777);
        profile.batching.output_expansion_factor = Some(1.9);
        profile.batching.max_units_per_batch = Some(11);
        let args = parse_args(&[]);
        apply_batching_overrides(&mut profile, &args);
        assert_eq!(profile.batching.target_output_tokens, Some(7777));
        assert_eq!(profile.batching.output_expansion_factor, Some(1.9));
        assert_eq!(profile.batching.max_units_per_batch, Some(11));
    }

    /// DCR-0026: `--table-strategy` joins the batching flags under the same
    /// precedence rule — flag > profile > built-in default — and clap refuses
    /// anything that is not one of the two documented values.
    #[test]
    fn table_strategy_follows_flag_then_profile_then_default() {
        // Built-in default: the shipped profile splits.
        let mut profile = default_profile();
        apply_batching_overrides(&mut profile, &parse_args(&[]));
        assert_eq!(
            profile.constraints.default_table_strategy.as_deref(),
            Some("row-window-first"),
            "the shipped default"
        );

        // Profile beats the default, and an absent flag preserves it.
        let mut profile = default_profile();
        profile.constraints.default_table_strategy = Some("whole-block".to_string());
        apply_batching_overrides(&mut profile, &parse_args(&[]));
        assert_eq!(
            profile.constraints.default_table_strategy.as_deref(),
            Some("whole-block")
        );

        // Flag beats the profile, in both directions.
        let mut profile = default_profile();
        profile.constraints.default_table_strategy = Some("row-window-first".to_string());
        apply_batching_overrides(
            &mut profile,
            &parse_args(&["--table-strategy", "whole-block"]),
        );
        assert_eq!(
            profile.constraints.default_table_strategy.as_deref(),
            Some("whole-block")
        );
        let mut profile = default_profile();
        profile.constraints.default_table_strategy = Some("whole-block".to_string());
        apply_batching_overrides(
            &mut profile,
            &parse_args(&["--table-strategy", "row-window-first"]),
        );
        assert_eq!(
            profile.constraints.default_table_strategy.as_deref(),
            Some("row-window-first")
        );

        // And an unrecognized value never reaches the profile at all.
        assert!(
            Harness::try_parse_from([
                "transync",
                "--input",
                "in.md",
                "--target-language",
                "ko",
                "--table-strategy",
                "by-vibes",
            ])
            .is_err(),
            "clap rejects a value outside the documented pair"
        );
    }

    #[test]
    fn expansion_factor_parser_rejects_non_positive_and_non_finite() {
        assert!(parse_expansion_factor("0").is_err());
        assert!(parse_expansion_factor("-1.5").is_err());
        assert!(parse_expansion_factor("nan").is_err());
        assert!(parse_expansion_factor("inf").is_err());
        assert!(parse_expansion_factor("abc").is_err());
        assert_eq!(parse_expansion_factor("1.4").unwrap(), 1.4);
        assert_eq!(
            parse_expansion_factor("0.8").unwrap(),
            0.8,
            "sub-1.0 is legal"
        );
    }

    /// OI-0026: the opt-in/opt-out pair maps to the pipeline's tri-state,
    /// and asking for both at once is a usage error (clap), not a silent
    /// precedence decision.
    #[test]
    fn auto_glossary_flags_set_option_and_conflict() {
        assert_eq!(resolve_auto_glossary_flag(&parse_args(&[])), None);
        assert_eq!(
            resolve_auto_glossary_flag(&parse_args(&["--auto-glossary"])),
            Some(true)
        );
        assert_eq!(
            resolve_auto_glossary_flag(&parse_args(&["--no-auto-glossary"])),
            Some(false),
            "an explicit opt-out must be expressible so it can beat a profile that enables it"
        );

        let both = Harness::try_parse_from([
            "transync",
            "--input",
            "in.md",
            "--target-language",
            "ko",
            "--auto-glossary",
            "--no-auto-glossary",
        ]);
        assert!(
            both.is_err(),
            "--auto-glossary and --no-auto-glossary must conflict at parse time"
        );
    }

    /// R0001-0021: the padding comes off once, here, so the prompt compiler,
    /// the cache key, the alignment map and the bundle's `lang` attribute all
    /// see the same canonical label. ` auto ` in particular has to reach the
    /// pipeline as the sentinel it was typed as.
    #[test]
    fn language_labels_are_normalized_at_the_boundary() {
        let args = parse_args_with_target(" ko\t", &["--source-language", " auto "]);
        let labels = resolve_language_and_model_args(&args).expect("padded labels are valid");
        assert_eq!(labels.source, "auto");
        assert_eq!(labels.target, "ko");

        let bare = parse_args(&["--source-language", "auto"]);
        let bare_labels = resolve_language_and_model_args(&bare).expect("valid");
        assert_eq!(
            (labels.source, labels.target),
            (bare_labels.source, bare_labels.target),
            "a padded run must be indistinguishable from the bare one"
        );
    }

    /// ti fd5aa8: the sentinel is a reserved literal, so a recognized one
    /// reaches the pipeline in its canonical spelling however it was typed.
    /// Before the fix `AUTO` compiled the auto-detection prompt and rendered
    /// the source pane from the *detected* label, yet reached the cache key
    /// and the alignment map as the literal `AUTO` — one run with two answers
    /// about which identity it has.
    #[test]
    fn a_recognized_sentinel_is_stored_canonically() {
        let bare = resolve_language_and_model_args(&parse_args(&["--source-language", "auto"]))
            .expect("the bare sentinel is valid");

        for typed in ["AUTO", "Auto", "aUtO", " AUTO\t"] {
            let args = parse_args(&["--source-language", typed]);
            let labels = resolve_language_and_model_args(&args)
                .expect("the sentinel is valid however it is typed");
            assert_eq!(
                labels.source, SOURCE_LANGUAGE_SENTINEL,
                "{typed:?} asks for detection, so it must key, prompt and stamp as `auto`"
            );
            assert_eq!(
                labels.source, bare.source,
                "{typed:?} must be indistinguishable from the bare sentinel downstream"
            );
        }
    }

    /// The fold reaches the reserved literal and nothing else. A label the
    /// recognizer rejects keeps every byte including its case, and no literal
    /// is reserved on the *target* side at all — `render_prompt_body` tests
    /// only the source label — so `--target-language AUTO` stays `AUTO`.
    #[test]
    fn only_the_reserved_literal_is_folded() {
        for label in ["DE", "Auto-detect", "AUTOMATIC", "zh-Hant"] {
            let args = parse_args(&["--source-language", label]);
            let labels = resolve_language_and_model_args(&args).expect("valid");
            assert_eq!(
                labels.source, label,
                "a label the sentinel test rejects is opaque, case included"
            );
        }

        let args = parse_args_with_target("AUTO", &[]);
        let labels = resolve_language_and_model_args(&args).expect("valid");
        assert_eq!(
            labels.target, "AUTO",
            "`auto` is not reserved on the target side, so nothing is folded there"
        );
    }

    /// ADR-0013 is untouched: only the *surrounding* whitespace goes. An
    /// expressive label keeps its interior spaces, its case, and every
    /// non-ASCII byte, because the label is opaque and this is not validation.
    #[test]
    fn an_expressive_label_survives_normalization_intact() {
        let args = parse_args_with_target("  Korean (formal, 존댓말)  ", &[]);
        let labels = resolve_language_and_model_args(&args).expect("valid");
        assert_eq!(labels.target, "Korean (formal, 존댓말)");
    }

    /// R0008-0039 still holds through the rewrite: a whitespace-only label is
    /// blank, and an explicit empty `--model` must not fall through to the
    /// default.
    #[test]
    fn blank_identifiers_are_still_rejected() {
        assert!(resolve_language_and_model_args(&parse_args_with_target("   ", &[])).is_err());
        assert!(
            resolve_language_and_model_args(&parse_args(&["--source-language", "\t"])).is_err()
        );
        assert!(resolve_language_and_model_args(&parse_args(&["--model", " "])).is_err());
        assert!(resolve_language_and_model_args(&parse_args(&[])).is_ok());
    }

    /// ti 0f26b5: the bundle title resolves **flag > first H1 > `transync`**,
    /// and each level is reachable — a flag beats a document that has a title,
    /// the document's title is used when no flag is given, and the literal is
    /// what a document with no H1 falls back to.
    #[test]
    fn bundle_title_resolves_flag_then_h1_then_literal() {
        assert_eq!(
            resolve_bundle_title(Some("Release Notes"), Some("Design Notes")),
            "Release Notes",
            "an explicit --title beats the document's own heading"
        );
        assert_eq!(
            resolve_bundle_title(None, Some("Design Notes")),
            "Design Notes",
            "with no flag, the document's first H1 titles the bundle"
        );
        assert_eq!(
            resolve_bundle_title(None, None),
            FALLBACK_BUNDLE_TITLE,
            "a document with no H1 falls back to the literal"
        );
        assert_eq!(
            resolve_bundle_title(Some("Release Notes"), None),
            "Release Notes",
            "the flag does not need a document heading to beat"
        );
    }

    /// A heading that renders to nothing (`#` alone, or one holding only
    /// markup) is not a title, so it must not produce `<title></title>` — the
    /// blank falls through to the literal. The document title is untrusted
    /// content, so this is a shape the CLI actually meets.
    #[test]
    fn a_blank_document_heading_is_not_a_title() {
        assert_eq!(resolve_bundle_title(None, Some("")), FALLBACK_BUNDLE_TITLE);
        assert_eq!(
            resolve_bundle_title(None, Some("  \t ")),
            FALLBACK_BUNDLE_TITLE
        );
        assert_eq!(
            resolve_bundle_title(None, Some("  Spaced Out  ")),
            "Spaced Out",
            "a usable heading is trimmed, not rejected"
        );
    }

    /// The flag half is normalized and checked at the argument boundary: an
    /// absent flag is `None`, a padded one is trimmed, and a blank one is an
    /// argument error rather than a silent fall-through to the H1/literal
    /// levels — the same posture an explicit empty `--model` gets.
    #[test]
    fn the_title_flag_is_trimmed_and_a_blank_one_is_rejected() {
        assert_eq!(resolve_title_flag(&parse_args(&[])), Ok(None));
        assert_eq!(
            resolve_title_flag(&parse_args(&["--title", "  Design Notes\t"])),
            Ok(Some("Design Notes".to_string()))
        );
        assert!(resolve_title_flag(&parse_args(&["--title", ""])).is_err());
        assert!(resolve_title_flag(&parse_args(&["--title", "   "])).is_err());
    }

    /// Regression for the flag-precedence sentinel hole: an explicit
    /// `--target-input-tokens-per-batch` equal to the built-in default (6000)
    /// must still beat a profile's different value. Writing the flag onto
    /// `TranslateOptions` (the pre-fix behavior) made it indistinguishable
    /// from unset, so the profile's 4000 silently won.
    #[test]
    fn explicit_input_tokens_flag_equal_to_default_beats_profile() {
        let mut profile = default_profile();
        profile.batching.target_input_tokens_per_batch = Some(4000);
        let default_input = TranslateOptions::default().target_input_tokens_per_batch;
        let args = parse_args(&[
            "--target-input-tokens-per-batch",
            &default_input.to_string(),
        ]);
        apply_batching_overrides(&mut profile, &args);
        assert_eq!(
            profile.batching.target_input_tokens_per_batch,
            Some(default_input),
            "an explicit flag equal to the built-in default must override the profile"
        );
    }
}
