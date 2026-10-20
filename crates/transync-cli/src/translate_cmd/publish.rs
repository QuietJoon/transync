//! Output-set assembly and commit for `transync translate`.
//!
//! Both output modes publish the same *kind* of set — the translated document,
//! alignment map, optional validation report, optional HTML bundle — so the
//! set is built once here and only the destinations and the commit function
//! differ. `--out-dir` commits the whole directory through `publish_out_dir`;
//! individual `--output`/`--map` paths commit as one staged fileset.
//!
//! TRACE: R0006-0012
//! TRACE: EXT-2026-07 P1-6

use super::TranslateArgs;
use super::args::InputFormatArg;
use super::args::{LanguageLabels, OutputTarget, is_source_language_sentinel};
use super::report::Reporter;
use crate::direction;
use crate::output::{
    EntryIdentity, html_bundle_files, html_bundle_paths, preflight_destination_set,
    preflight_html_out, preflight_out_dir, publish_out_dir, write_fileset_atomic,
};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use transync::TranslationOutput;

/// Failure modes of [`publish_outputs`]. Each variant carries what the
/// caller needs to render its diagnostic; the exit-code mapping stays with
/// the rest of the command's exit-code policy in `translate_cmd::execute`.
pub(crate) enum PublishError {
    SerializeAlignment(serde_json::Error),
    SerializeReport(serde_json::Error),
    HtmlOutPreflight {
        dir: PathBuf,
        source: std::io::Error,
    },
    Commit(std::io::Error),
}

/// Where each member of the output set lands, and how the set is committed.
/// The only per-mode decisions: the four destination paths, whether the
/// validation report and the HTML bundle are part of the set, and which
/// commit function runs.
struct Destinations<'a> {
    document: PathBuf,
    map: PathBuf,
    /// `Some` when the validation report belongs to the set — always under
    /// `--out-dir`, only with `--validation-report` otherwise.
    report: Option<PathBuf>,
    /// `Some` when the HTML bundle belongs to the set, holding the directory
    /// its six files are written under (relative to the published dir for
    /// `--out-dir`, absolute-or-relative to CWD for `--html-out`).
    bundle_dir: Option<PathBuf>,
    commit: Commit<'a>,
}

/// The commit function each output mode uses.
enum Commit<'a> {
    /// `--out-dir`: one directory-granularity publish. The foreign-file guard
    /// lives inside `publish_out_dir` (it guards the target, not the staged
    /// dir), so no separate preflight runs here.
    Dir(&'a Path),
    /// Individual `--output`/`--map` (plus `--html-out` and
    /// `--validation-report`), each committed as its own staged file.
    Files,
}

/// Answer every question about the *destinations* that does not need the
/// translated content, before the provider is called (R0002-0029).
///
/// A foreign `--html-out` directory, an `--out-dir` target that is not a prior
/// transync out-dir, a destination set naming one file twice — under the
/// filesystem's own name equivalence, so `OUT.md` beside `out.md` on a volume
/// that folds letter case (R0012-0002) — a destination that is another one's
/// staging name (R0013-0001), and a destination whose staging name the
/// filesystem refuses (R0013-0006) are all
/// decided by the filesystem and the arguments alone. Learning any of them
/// after a paid translation run — the run's whole cost, unsalvageable because
/// the cache is per-run and in-memory — is the expensive way to be told
/// something that was true before the run started. It is the same reasoning
/// `translate_cmd::execute` already applies to `--title ''` and a missing
/// `--output`/`--map` pair.
///
/// **Advisory, not authoritative.** Every guard here runs again inside
/// [`publish_outputs`] — for `--out-dir`, under the publication lock, which is
/// where the answer has to be current — because anything checked outside that
/// lock can be stale by the time the rename runs. Nothing is skipped later on
/// the strength of this pass, and this pass says nothing on stderr: the
/// authoritative pass is the one that reports queued publications and
/// preserved foreign staging temps, and one run should not print those twice.
///
/// The read-side half (R0010-0003 / R0010-0004) is asked **only** here, and it
/// is the one guard in this function that is not repeated under the lock: it
/// compares argv against argv, so a second answer would differ from this one
/// only if the operator moved the run's own input files mid-run — and paying
/// for that with a discarded translation buys nothing the refusal below has not
/// already bought before the provider was called.
pub(crate) fn preflight_destinations(
    target: &OutputTarget,
    args: &TranslateArgs,
) -> Result<(), PublishError> {
    let quiet = |_: &str| {};
    match target {
        OutputTarget::Dir { dir } => {
            preflight_out_dir(dir, args.force, &quiet).map_err(PublishError::Commit)?;
            refuse_read_side_under(dir, args)
        }
        OutputTarget::Files {
            output: out_path,
            map: map_path,
        } => {
            let mut destinations = vec![out_path.clone(), map_path.clone()];
            destinations.extend(args.validation_report.clone());
            if let Some(dir) = &args.html_out {
                preflight_html_out(dir, args.force, &quiet).map_err(|e| {
                    PublishError::HtmlOutPreflight {
                        dir: dir.clone(),
                        source: e,
                    }
                })?;
                destinations.extend(html_bundle_paths(dir));
            }
            preflight_destination_set(&destinations).map_err(PublishError::Commit)?;
            refuse_read_side_aliases(&destinations, args)
        }
    }
}

/// The files a run **reads**, each with the flag that named it and the entry
/// it identifies — a path that resolves to nothing is left out, since that run
/// is about to fail its own read anyway.
///
/// A read-side path is canonicalized **whole**, final component included,
/// because the run reads *through* every symlink on it: what must survive is
/// the file at the end of the chain, not the entry that pointed at it. The
/// result then goes through the same [`EntryIdentity`] as every destination,
/// so both sides name the final component the way its directory stores it.
/// macOS's `realpath` already spells the read side that way; Linux's keeps the
/// typed spelling on a casefolded directory, and comparing one side
/// normalized against the other raw is exactly the gap R0012-0002 names.
fn read_side_identities<'a>(
    args: &'a TranslateArgs,
    cwd: &Path,
) -> Vec<(&'static str, &'a Path, EntryIdentity)> {
    [
        Some(("--input", args.input.as_path())),
        args.profile.as_deref().map(|p| ("--profile", p)),
        args.system_prompt_file
            .as_deref()
            .map(|p| ("--system-prompt-file", p)),
    ]
    .into_iter()
    .flatten()
    .filter_map(|(flag, path)| {
        let read = fs::canonicalize(path).ok()?;
        Some((flag, path, EntryIdentity::of(cwd, &read)))
    })
    .collect()
}

/// The working directory the identities are made absolute against, on the
/// destination refusal's channel.
fn working_dir() -> Result<PathBuf, PublishError> {
    std::env::current_dir().map_err(PublishError::Commit)
}

/// Refuse a destination that names a file this run reads (R0010-0003,
/// R0010-0004).
///
/// `vet_destinations` asks whether the output set names one file twice;
/// `--output <the --input>` names one file twice across the read/write divide,
/// and the outcome is worse than a mangled fileset. The publication renames the
/// staged translation over the document the run was translating, or over the
/// `--profile` / `--system-prompt-file` that governed it, and nothing reports
/// it: the source is already in memory and the write is a legitimate
/// publication of a successful run.
///
/// **`--force` does not waive this.** That flag says an existing *output* may
/// be replaced — a foreign `--html-out` file, an `--out-dir` target that is not
/// a prior out-dir — and no argv spelling asks for a run to destroy its own
/// inputs.
///
/// "Names" is the filesystem's question (R0012-0002): a destination is the
/// entry [`EntryIdentity`] resolves, so `--output IN.md` is `--input in.md` on
/// a volume that folds letter case, while an `alias.md` that *symlinks* the
/// input is a different entry — publication renames over the link rather than
/// through it — and so is a hard link of it.
fn refuse_read_side_aliases(
    destinations: &[PathBuf],
    args: &TranslateArgs,
) -> Result<(), PublishError> {
    let cwd = working_dir()?;
    let reads = read_side_identities(args, &cwd);
    for destination in destinations {
        let written = EntryIdentity::of(&cwd, destination);
        for (flag, path, read) in &reads {
            if read.same_entry(&written) {
                return Err(read_side_refusal(format!(
                    "output destination {} is the {flag} file {} this run reads: publishing it \
                     would overwrite the run's own input",
                    destination.display(),
                    path.display(),
                )));
            }
        }
    }
    Ok(())
}

/// [`refuse_read_side_aliases`] for `--out-dir`, whose destination is the whole
/// target: a publication replaces the directory, so an input *inside* it is
/// destroyed without any destination naming that input — and an input that
/// *is* the target (a regular file the publication would replace) is the same
/// loss.
///
/// Inside is asked of the filesystem as well as of the spelling
/// ([`EntryIdentity::is_inside`]): the target's `dev`+`ino` among the
/// directories the input's path runs through, so an `--out-dir` spelled in
/// another letter case is caught on a volume whose `realpath` keeps the typed
/// spelling (a casefolded Linux directory), not only where it does not.
fn refuse_read_side_under(dir: &Path, args: &TranslateArgs) -> Result<(), PublishError> {
    let Ok(published) = fs::canonicalize(dir) else {
        return Ok(());
    };
    let cwd = working_dir()?;
    let published = EntryIdentity::of(&cwd, &published);
    for (flag, path, read) in read_side_identities(args, &cwd) {
        if read.same_entry(&published) || read.is_inside(&published) {
            return Err(read_side_refusal(format!(
                "--out-dir {} holds the {flag} file {} this run reads, and publishing replaces \
                 the whole target directory",
                dir.display(),
                path.display(),
            )));
        }
    }
    Ok(())
}

/// The same channel every other destination refusal takes, so an operator
/// cannot tell the read-side guard apart from the duplicate-destination one by
/// its exit code (`contracts.md` §6).
fn read_side_refusal(message: String) -> PublishError {
    PublishError::Commit(io::Error::new(io::ErrorKind::InvalidInput, message))
}

/// Serialize, preflight, assemble, and commit the whole output set.
///
/// R0006-0012: everything is serialized and preflighted BEFORE the first
/// write, then committed through one staged commit — so a failure while the
/// content is being written leaves every destination untouched.
///
/// **That is the guarantee, and it is not whole-set atomicity** (R0004-0009).
/// ADR-0006's "atomic" is per *file*: `<path>.tmp.<pid>` → fsync → rename, one
/// destination at a time. In files mode (`--output` / `--map` / `--html-out`)
/// the commit therefore has two phases, and only the first is all-or-nothing:
/// staging every payload (the I/O, the seconds) rolls back completely, while
/// the phase-2 rename pass walks the set. A crash or an I/O error inside that
/// pass leaves the destinations it already renamed carrying this run's output
/// and the rest carrying whatever they held before — a new `out.md` beside the
/// previous run's bundle. The exposure is a few metadata renames rather than
/// the whole write; a concurrent run cannot land in it, because publications
/// into a directory serialize on the publish lock; and what the cleanup could
/// not remove is reported. `--out-dir` is the mode that commits the whole set
/// at once — one directory rename — and is the answer for a consumer that
/// needs set-level atomicity. `contracts.md` §6 states both.
///
/// `languages` carries the run's labels as the pipeline received them —
/// normalized once at the argument boundary (R0001-0021) — so the bundle's
/// `lang` attributes and the alignment map cannot disagree about the label.
///
/// `bundle_title` is the already-resolved `<title>` text (flag > document H1 >
/// `transync`; ti 0f26b5). It arrives resolved rather than as the two inputs
/// because only one of them is an argument: the H1 half does not exist until
/// the pipeline has parsed the document, and the flag half was validated
/// before the run started.
///
/// `reporter` is the writer's notice channel (R0001-0034 / R0001-0036 /
/// R0002-0025): the publish layer has three facts only it can know — that it
/// is queued behind another run's publication, that it is leaving another
/// run's staging temps in place, and that a directory's flush to disk failed
/// (the published file contents are durable, the entries naming them may not
/// survive a crash) — and they belong on the same `--quiet`-gated stderr
/// stream as every other `transync: ` line.
pub(crate) fn publish_outputs(
    target: &OutputTarget,
    args: &TranslateArgs,
    languages: &LanguageLabels,
    bundle_title: &str,
    output: &TranslationOutput,
    profile_target_direction: Option<&str>,
    reporter: &Reporter,
) -> Result<(), PublishError> {
    let notify = |line: &str| reporter.say(line);
    let alignment_json = serde_json::to_vec_pretty(&output.alignment_map)
        .map_err(PublishError::SerializeAlignment)?;

    let dest = match target {
        // --out-dir: out.md, alignment.json, validation-report.json (ALWAYS
        // written in this mode), and the six-file bundle under html/, all
        // published into one directory.
        OutputTarget::Dir { dir } => Destinations {
            // §9/§6: out.html for an HTML run, out.md for a Markdown run —
            // one of the two, never both. Routing is flag-only, so the flag
            // is the authority here too. Exhaustive by charter.
            document: PathBuf::from(match args.input_format {
                InputFormatArg::Markdown => "out.md",
                InputFormatArg::Html => "out.html",
            }),
            map: PathBuf::from("alignment.json"),
            report: Some(PathBuf::from("validation-report.json")),
            bundle_dir: Some(PathBuf::from("html")),
            commit: Commit::Dir(dir),
        },
        OutputTarget::Files {
            output: out_path,
            map: map_path,
        } => Destinations {
            document: out_path.clone(),
            map: map_path.clone(),
            report: args.validation_report.clone(),
            bundle_dir: args.html_out.clone(),
            commit: Commit::Files,
        },
    };

    // Only a free-standing --html-out directory needs its own up-front guard;
    // an --out-dir publish guards its whole target inside publish_out_dir.
    if matches!(dest.commit, Commit::Files)
        && let Some(dir) = &dest.bundle_dir
    {
        preflight_html_out(dir, args.force, &notify).map_err(|e| {
            PublishError::HtmlOutPreflight {
                dir: dir.clone(),
                source: e,
            }
        })?;
    }

    let report_json: Vec<u8> = match &dest.report {
        Some(_) => serde_json::to_vec_pretty(&output.validation_report)
            .map_err(PublishError::SerializeReport)?,
        None => Vec::new(),
    };

    let bundle: Vec<(PathBuf, Vec<u8>)> = match &dest.bundle_dir {
        Some(dir) => {
            let src_lang = pane_source_language(
                &languages.source,
                &output.alignment_map.detected_source_language,
            );
            // OI-0032: pane text direction for the HTML bundle. The target
            // pane resolves flag > profile > auto; the source pane always
            // auto-resolves from its machine-resolved label (there is no
            // --source-direction flag — the label is not user prose, and the
            // auto table covers it). LTR emits no attribute, so ko/ja/en
            // bundles stay byte-identical to pre-OI-0032 output.
            let target_mode =
                direction::resolve_mode(args.target_direction, profile_target_direction);
            html_bundle_files(
                dir,
                &output.annotated_source_html,
                &output.annotated_target_html,
                &alignment_json,
                bundle_title,
                src_lang,
                &languages.target,
                direction::dir_attr(direction::DirectionMode::Auto, src_lang),
                direction::dir_attr(target_mode, &languages.target),
                args.strict_csp,
            )
        }
        None => {
            // OI-0018: the bundle shell is the only thing --strict-csp acts
            // on. A security flag that silently does nothing is worse than a
            // noisy one, so a run that emits no bundle says so rather than
            // leaving the operator believing the outputs are hardened.
            //
            // R0010-0059: `--title` and `--target-direction` are bundle-only in
            // exactly the same way — neither reaches out.md, the alignment map
            // or the provider — so they take the same sentence rather than a
            // second mechanism. A note and not an argument error, because the
            // combination is inert rather than contradictory.
            for flag in bundle_only_flags_given(args) {
                notify(&format!(
                    "note: {flag} has no effect without --html-out or --out-dir (this run emits \
                     no HTML bundle)"
                ));
            }
            Vec::new()
        }
    };

    let mut files: Vec<(PathBuf, &[u8])> = vec![
        (dest.document, output.translated_document.as_bytes()),
        (dest.map, alignment_json.as_slice()),
    ];
    if let Some(path) = dest.report {
        files.push((path, report_json.as_slice()));
    }
    files.extend(bundle.iter().map(|(p, b)| (p.clone(), b.as_slice())));

    match dest.commit {
        Commit::Dir(dir) => publish_out_dir(dir, &files, args.force, &notify),
        Commit::Files => write_fileset_atomic(&files, &notify),
    }
    .map_err(PublishError::Commit)
}

/// The bundle-only flags this run passed, in the order the advisory names
/// them. Each is consumed by bundle assembly and nowhere else, so a run that
/// emits no bundle consumed none of them (R0010-0059).
fn bundle_only_flags_given(args: &TranslateArgs) -> Vec<&'static str> {
    let mut given = Vec::new();
    if args.strict_csp {
        given.push("--strict-csp");
    }
    if args.title.is_some() {
        given.push("--title");
    }
    if args.target_direction.is_some() {
        given.push("--target-direction");
    }
    given
}

/// The language label to stamp on the generated bundle's source pane
/// (R0008-0050). `--source-language auto` carries no usable code, so fall
/// back to the model's detected language when the run produced one, else an
/// empty label (no `lang` attribute emitted).
///
/// R0001-0021: `requested` arrives already trimmed from
/// [`super::args::resolve_language_and_model_args`], so no normalizing is left
/// to do here. The `.trim()` that used to sit here was the *only* place
/// padding was absorbed, which is exactly what made this pane disagree with
/// the prompt, the cache key and the alignment map about whether ` auto ` was
/// the sentinel.
///
/// ti fd5aa8: the question "is this the sentinel?" is asked through
/// [`is_source_language_sentinel`] rather than answered again locally, so this
/// pane and the boundary that canonicalized the label cannot drift apart. The
/// recognizer stays case-insensitive even though the boundary now hands over a
/// canonical `auto`: a caller that reached this function some other way still
/// gets detection rather than a `lang="AUTO"` on the pane.
fn pane_source_language<'a>(requested: &'a str, detected: &'a Option<String>) -> &'a str {
    if is_source_language_sentinel(requested) {
        detected.as_deref().unwrap_or("")
    } else {
        requested
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::testing::{folds_letter_case, plant_with_normalization_alias, scratch_dir};
    use clap::Parser;

    /// Constructing `TranslateArgs` through clap keeps these tests robust to
    /// future field additions (defaults fill in) — the same harness rule
    /// `args`'s tests follow.
    #[derive(Parser)]
    struct Harness {
        #[command(flatten)]
        args: TranslateArgs,
    }

    fn parse_args(extra: &[&str]) -> TranslateArgs {
        let mut argv = vec!["transync", "--target-language", "ko"];
        argv.extend_from_slice(extra);
        Harness::try_parse_from(argv)
            .expect("args should parse")
            .args
    }

    fn refusal(e: PublishError) -> String {
        match e {
            PublishError::Commit(e) => e.to_string(),
            _ => panic!("a destination refusal arrives as PublishError::Commit"),
        }
    }

    /// R0010-0003 / R0010-0004: a destination that names a file this run READS
    /// is refused before the provider is called, `--force` included — that flag
    /// governs replaceable outputs, never the run's own inputs. The alias is
    /// spelled through a `.` component so the refusal is identity-based rather
    /// than textual, the way the output-set duplicate check already is.
    #[test]
    fn a_destination_that_names_a_read_side_file_is_refused() {
        let root = scratch_dir("read-side-alias");
        let input = root.join("in.md");
        fs::write(&input, "# hi\n").expect("fixture input");
        let profile = root.join("profile.toml");
        fs::write(&profile, "").expect("fixture profile");

        let aliased_input = root.join(".").join("in.md");
        let args = parse_args(&[
            "--input",
            input.to_str().expect("scratch paths are UTF-8"),
            "--profile",
            profile.to_str().expect("scratch paths are UTF-8"),
            "--force",
        ]);
        let onto_input = OutputTarget::Files {
            output: aliased_input,
            map: root.join("alignment.json"),
        };
        let message = refusal(
            preflight_destinations(&onto_input, &args)
                .expect_err("--output naming --input must be refused"),
        );
        assert!(
            message.contains("--input") && message.contains("in.md"),
            "the refusal must name the read-side flag and its file: {message}"
        );

        // The profile and the system prompt file are read the same way, so the
        // same alias through --map is the same refusal.
        let onto_profile = OutputTarget::Files {
            output: root.join("out.md"),
            map: root.join("profile.toml"),
        };
        let message = refusal(
            preflight_destinations(&onto_profile, &args)
                .expect_err("--map naming --profile must be refused"),
        );
        assert!(
            message.contains("--profile"),
            "the refusal must name the read-side flag: {message}"
        );

        // And a set that aliases nothing still publishes.
        let clean = OutputTarget::Files {
            output: root.join("out.md"),
            map: root.join("alignment.json"),
        };
        assert!(
            preflight_destinations(&clean, &args).is_ok(),
            "a destination set that names no read-side file must pass"
        );
    }

    /// R0010-0003: `--out-dir` needs no destination to name the input — the
    /// publication replaces the whole target, so an input *inside* it is
    /// destroyed by a set that names only `out.md` and `alignment.json`.
    #[test]
    fn an_out_dir_holding_a_read_side_file_is_refused() {
        let root = scratch_dir("read-side-out-dir");
        let published = root.join("published");
        fs::create_dir(&published).expect("an existing --out-dir target");
        let input = published.join("in.md");
        fs::write(&input, "# hi\n").expect("fixture input");

        let args = parse_args(&[
            "--input",
            input.to_str().expect("scratch paths are UTF-8"),
            "--force",
        ]);
        let message = refusal(
            preflight_destinations(&OutputTarget::Dir { dir: published }, &args)
                .expect_err("an --out-dir holding the input must be refused"),
        );
        assert!(
            message.contains("--input"),
            "the refusal must name the read-side flag: {message}"
        );

        // A sibling directory holds nothing this run reads. `publishedX` also
        // pins that the containment test is component-wise.
        let sibling = root.join("publishedX");
        fs::create_dir(&sibling).expect("a sibling target");
        assert!(
            preflight_destinations(&OutputTarget::Dir { dir: sibling }, &args).is_ok(),
            "a target that holds no read-side file must pass"
        );
    }

    /// R0012-0002: on a volume that folds letter case, `--output IN.md` is the
    /// `--input in.md` file, and the guard compared the input's realpath
    /// (which spells the stored `in.md`) with the destination's final
    /// component *as typed* — so the translation was renamed over the source
    /// with nothing reported, `--input IN.MD --output IN.MD` included. Both
    /// sides now name the leaf the way its directory stores it. A symlink to
    /// the input and a hard link of it are still separate entries the
    /// publication renames over rather than through, and still publish.
    #[test]
    fn a_case_alias_of_a_read_side_file_is_refused() {
        let root = scratch_dir("read-side-case-alias");
        if !folds_letter_case(&root) {
            eprintln!(
                "note: {} is case-sensitive; R0012-0002's read-side case half did not run on \
                 this filesystem",
                root.display()
            );
            return;
        }
        let input = root.join("in.md");
        fs::write(&input, "# hi\n").expect("fixture input");
        let profile = root.join("profile.toml");
        fs::write(&profile, "").expect("fixture profile");
        let spelled = |path: &Path| path.to_str().expect("scratch paths are UTF-8").to_string();
        let args = parse_args(&[
            "--input",
            &spelled(&input),
            "--profile",
            &spelled(&profile),
            "--force",
        ]);

        let onto_input = OutputTarget::Files {
            output: root.join("IN.md"),
            map: root.join("alignment.json"),
        };
        let message = refusal(
            preflight_destinations(&onto_input, &args)
                .expect_err("--output naming --input in another letter case must be refused"),
        );
        assert!(message.contains("--input"), "{message}");

        let onto_profile = OutputTarget::Files {
            output: root.join("out.md"),
            map: root.join("PROFILE.TOML"),
        };
        let message = refusal(
            preflight_destinations(&onto_profile, &args)
                .expect_err("--map naming --profile in another letter case must be refused"),
        );
        assert!(message.contains("--profile"), "{message}");

        // The identical spelling of an alias: the input's realpath spells the
        // stored `in.md`, the destination kept `IN.MD`.
        let shouted = root.join("IN.MD");
        let same_spelling = parse_args(&["--input", &spelled(&shouted)]);
        let message = refusal(
            preflight_destinations(
                &OutputTarget::Files {
                    output: shouted,
                    map: root.join("alignment.json"),
                },
                &same_spelling,
            )
            .expect_err("--input IN.MD --output IN.MD names one file twice"),
        );
        assert!(message.contains("--input"), "{message}");

        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&input, root.join("link.md")).expect("symlink the input");
            fs::hard_link(&input, root.join("hard.md")).expect("hard-link the input");
            for separate in ["LINK.MD", "hard.md", "HARD.MD"] {
                preflight_destinations(
                    &OutputTarget::Files {
                        output: root.join(separate),
                        map: root.join("alignment.json"),
                    },
                    &args,
                )
                .unwrap_or_else(|e| {
                    panic!("{separate} is an entry of its own: {}", refusal(e));
                });
            }
        }

        let published = root.join("published");
        fs::create_dir(&published).expect("an existing --out-dir target");
        let inside = published.join("in.md");
        fs::write(&inside, "# hi\n").expect("an input inside the target");
        let message = refusal(
            preflight_destinations(
                &OutputTarget::Dir {
                    dir: root.join("PUBLISHED"),
                },
                &parse_args(&["--input", &spelled(&inside), "--force"]),
            )
            .expect_err("an --out-dir spelled in another letter case still holds the input"),
        );
        assert!(message.contains("--input"), "{message}");
    }

    /// R0012-0002, the normalization half, with the alias derived from the
    /// spelling the directory stores (see `plant_with_normalization_alias`):
    /// on HFS+ the input's realpath spells NFD and the destination NFC, on
    /// APFS the other way round — either way the comparison was of two byte
    /// strings naming one file.
    #[test]
    fn a_normalization_alias_of_a_read_side_file_is_refused() {
        let root = scratch_dir("read-side-normalization-alias");
        let Some((stored, alias)) = plant_with_normalization_alias(&root, ".md") else {
            eprintln!(
                "note: {} does not fold Unicode normalization; R0012-0002's read-side \
                 normalization half did not run on this filesystem",
                root.display()
            );
            return;
        };
        for (input, output) in [(&stored, &alias), (&alias, &alias), (&alias, &stored)] {
            let args = parse_args(&["--input", input.to_str().expect("scratch paths are UTF-8")]);
            let message = refusal(
                preflight_destinations(
                    &OutputTarget::Files {
                        output: output.clone(),
                        map: root.join("alignment.json"),
                    },
                    &args,
                )
                .expect_err("a normalization alias of --input is --input"),
            );
            assert!(message.contains("--input"), "{message}");
        }
    }

    /// R0013-0001 through the CLI's own preflight: the test process's pid is
    /// the writer's, so `--output <dir>/out.json.tmp.<pid> --map <dir>/out.json`
    /// is exactly the pair an `exec`-ing shell wrapper could produce — and it is
    /// refused here, before the provider call, rather than published wrong.
    #[test]
    fn a_staging_name_of_this_run_is_refused_before_the_provider_call() {
        let root = scratch_dir("staging-name-preflight");
        let input = root.join("in.md");
        fs::write(&input, "# hi\n").expect("fixture input");
        let args = parse_args(&["--input", input.to_str().expect("scratch paths are UTF-8")]);
        let message = refusal(
            preflight_destinations(
                &OutputTarget::Files {
                    output: root.join(format!("out.json.tmp.{}", std::process::id())),
                    map: root.join("out.json"),
                },
                &args,
            )
            .expect_err("a destination that is another's staging name must be refused"),
        );
        assert!(message.contains("staging name"), "{message}");
    }

    /// R0010-0059 / OI-0018: every bundle-only flag earns the same no-bundle
    /// sentence, not just the one that had it.
    #[test]
    fn bundle_only_flags_are_reported_when_no_bundle_is_emitted() {
        let none = parse_args(&["--input", "in.md"]);
        assert!(bundle_only_flags_given(&none).is_empty());

        let all = parse_args(&[
            "--input",
            "in.md",
            "--strict-csp",
            "--title",
            "Doc",
            "--target-direction",
            "rtl",
        ]);
        assert_eq!(
            bundle_only_flags_given(&all),
            ["--strict-csp", "--title", "--target-direction"]
        );
    }

    /// The sentinel hands the pane over to the model's detection, and an
    /// absent detection leaves the label empty (no `lang` attribute).
    ///
    /// ti fd5aa8: `AUTO` cannot arrive from the argument boundary any more —
    /// it is canonicalized to `auto` there, which is what makes the cache key
    /// and the alignment map agree with this pane. The uppercase case stays
    /// asserted because the recognizer is shared with that boundary: whatever
    /// the boundary would fold, this pane resolves the same way.
    #[test]
    fn the_sentinel_defers_to_the_detected_label() {
        let detected = Some("en".to_string());
        assert_eq!(pane_source_language("auto", &detected), "en");
        assert_eq!(pane_source_language("AUTO", &detected), "en");
        assert_eq!(pane_source_language("auto", &None), "");
    }

    /// Any other label is stamped verbatim — opaque, per ADR-0013 — and beats
    /// a detection the provider volunteered.
    #[test]
    fn an_explicit_label_is_stamped_verbatim() {
        let detected = Some("en".to_string());
        assert_eq!(pane_source_language("de", &detected), "de");
        assert_eq!(
            pane_source_language("Korean (formal, 존댓말)", &None),
            "Korean (formal, 존댓말)"
        );
    }
}
