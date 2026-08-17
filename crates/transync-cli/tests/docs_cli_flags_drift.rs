//! Guard against drift between the shipped clap surface and the two
//! documents that publish it as a usage block.
//!
//! `docs/architecture/contracts.md` §6 is the CLI argument contract and
//! `docs/Developer_Guide.md`'s "CLI reference" is the reader-facing copy of
//! it. Both are hand-maintained, so both rot the same two ways: a flag lands
//! in `translate_cmd::TranslateArgs` and no block names it (the guide had
//! drifted ten flags behind by ticket `57be8c`), or a block names a flag
//! clap never accepted (a rename that only landed in prose).
//!
//! The authority here is the binary itself: `transync <sub> --help` is what
//! clap derives from the argument structs, so it cannot be out of date with
//! the code the way a third document copying a second one can. Both
//! directions are checked, and both blocks must name every flag of **both**
//! subcommands, which is what they each already set out to do.
//!
//! `--help` is excluded — it is clap's own, and no block claims it.
//!
//! One slice of each flag's *description* is welded too (ti fc0b18): the
//! default value. Descriptions are prose and stay hand-written and unchecked,
//! but where clap renders `[default: V]` on a help screen, V is a literal the
//! block's entry for that flag either carries or does not — no prose matching
//! involved. The subject set is scraped from the same help text, so a flag
//! clap gives no default (every `Option<T>` one, and every `bool` whose
//! `SetTrue` action prints none) is never asked about, and a flag that gains
//! or loses a default moves in and out of the check on its own.
//!
//! TRACE: ti 57be8c
//! TRACE: ti fc0b18
//! TRACE: contracts.md §6

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

/// The subcommands whose flags the blocks publish.
const SUBCOMMANDS: &[&str] = &["translate", "serve"];

/// Flags clap contributes itself; no usage block lists them.
const CLAP_OWN: &[&str] = &["--help", "--version"];

/// The guarded blocks: `(document, heading whose first fenced block is the
/// usage block)`.
const GUARDED: &[(&str, &str)] = &[
    (
        "docs/architecture/contracts.md",
        "## 6. CLI argument contract",
    ),
    ("docs/Developer_Guide.md", "## CLI reference"),
];

/// Repo root, derived from this crate's manifest dir (`crates/transync-cli`
/// -> `../..`). Same idiom as the docs drift tests in `crates/transync`.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root should resolve from CARGO_MANIFEST_DIR/../..")
}

fn read(rel: &str) -> String {
    let path = repo_root().join(rel);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{rel} should be readable ({}): {e}", path.display()))
}

/// `transync <sub> --help`, as clap renders it.
fn help_text(subcommand: &str) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_transync"))
        .args([subcommand, "--help"])
        .output()
        .unwrap_or_else(|e| panic!("`transync {subcommand} --help` should run: {e}"));
    assert!(
        out.status.success(),
        "`transync {subcommand} --help` should exit 0, got {:?}",
        out.status.code(),
    );
    String::from_utf8(out.stdout).expect("clap help should be UTF-8")
}

/// The long flags an option *header* line of a help screen declares
/// (`      --flag <VALUE>`, `  -h, --help`). Description and possible-value
/// lines yield nothing: only a token that is itself `--something` — the
/// first on the line, or the first after a `,` — contributes, so a flag
/// merely *mentioned* inside a doc comment is not mistaken for a declaration.
fn header_flags(line: &str) -> BTreeSet<String> {
    let mut flags = BTreeSet::new();
    let trimmed = line.trim_start();
    if !trimmed.starts_with('-') {
        return flags;
    }
    for part in trimmed.split(',') {
        let token = part.split_whitespace().next().unwrap_or("");
        let Some(name) = token.strip_prefix("--") else {
            continue;
        };
        if name.starts_with(|c: char| c.is_ascii_lowercase()) {
            flags.insert(format!("--{name}"));
        }
    }
    flags
}

/// Long flags clap declares, across a whole help screen.
fn declared_flags(help: &str) -> BTreeSet<String> {
    help.lines().flat_map(header_flags).collect()
}

/// The default clap renders on one help line, if any. Long help puts
/// `[default: V]` on a line of its own under the description; short help puts
/// it inline after it — either way it is the same bracketed literal, and a
/// quoted value is unquoted here so the blocks may state it either way.
fn default_value_in(line: &str) -> Option<String> {
    const MARKER: &str = "[default: ";
    let start = line.find(MARKER)? + MARKER.len();
    let rest = &line[start..];
    let end = rest.find(']')?;
    let value = rest[..end].trim().trim_matches('"').to_string();
    (!value.is_empty()).then_some(value)
}

/// `flag -> default` for every flag whose help entry carries a
/// `[default: V]`. A `[default: …]` line belongs to the header line above it,
/// which is how clap lays a help screen out.
fn declared_defaults(help: &str) -> BTreeMap<String, String> {
    let mut defaults = BTreeMap::new();
    let mut current = BTreeSet::new();
    for line in help.lines() {
        let header = header_flags(line);
        if !header.is_empty() {
            current = header;
        }
        let Some(value) = default_value_in(line) else {
            continue;
        };
        for flag in &current {
            defaults.insert(flag.clone(), value.clone());
        }
    }
    defaults
}

/// Every long flag named anywhere in `text`. A `--` run only opens a flag
/// when what precedes it cannot be part of a longer token, so `[--force]`,
/// `--quiet|--verbose` and `` `--map` `` all count while the `n >= 1` of a
/// prose clause does not.
fn flags_named_in(text: &str) -> BTreeSet<String> {
    let bytes = text.as_bytes();
    let mut flags = BTreeSet::new();
    let mut i = 0;
    while i + 2 < bytes.len() {
        let opens = bytes[i] == b'-'
            && bytes[i + 1] == b'-'
            && bytes[i + 2].is_ascii_lowercase()
            && (i == 0 || !(bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'-'));
        if !opens {
            i += 1;
            continue;
        }
        let mut end = i + 2;
        while end < bytes.len() && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'-') {
            end += 1;
        }
        // A trailing `-` belongs to the prose, not to the flag.
        let name = text[i..end].trim_end_matches('-');
        flags.insert(name.to_string());
        i = end;
    }
    flags
}

/// A usage block split into per-flag entries: `(the flags the entry heads,
/// the entry's full text)`.
///
/// An entry opens on a line that names a flag at the block's flag
/// indentation — the shallowest indentation any flag-naming line sits at,
/// which both blocks use for their flag lines and neither uses for
/// continuations — and runs until the next such line. Requiring that exact
/// indentation is what keeps a wrapped description that happens to begin with
/// `--map, and --html-out …` from opening an entry of its own. An entry is
/// keyed by the flags of its *leading token* only, so a flag named later in
/// the same line's prose does not inherit the entry.
fn flag_entries(block: &str) -> Vec<(BTreeSet<String>, String)> {
    fn indent(line: &str) -> usize {
        line.len() - line.trim_start().len()
    }
    fn heads_a_flag(line: &str) -> bool {
        let trimmed = line.trim_start();
        trimmed.starts_with("--") || trimmed.starts_with("[--")
    }
    let Some(flag_indent) = block.lines().filter(|l| heads_a_flag(l)).map(indent).min() else {
        return Vec::new();
    };
    let mut entries: Vec<(BTreeSet<String>, String)> = Vec::new();
    for line in block.lines() {
        if heads_a_flag(line) && indent(line) == flag_indent {
            let head = line.split_whitespace().next().unwrap_or("");
            entries.push((flags_named_in(head), String::new()));
        }
        if let Some((_, text)) = entries.last_mut() {
            text.push_str(line);
            text.push('\n');
        }
    }
    entries
}

/// An entry's text with its *signature* dropped: the `--flag` tokens and the
/// `<metavar>` sketches, leaving what the entry says about the flag.
///
/// A signature offers values without stating any, and
/// `[--source-language <label>|auto]` offers `auto` — so without this, a block
/// whose stated default had drifted to something else would still be answered
/// by its own syntax sketch, which is drift this check has to see.
fn stated_text(entry: &str) -> String {
    entry
        .split_whitespace()
        .filter(|token| {
            let bare = token.trim_matches(|c| "[](),".contains(c));
            !token.contains('<') && !token.contains('>') && !bare.starts_with("--")
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Whether `text` states `value` as a value of its own rather than as part of
/// a longer word or number: `7470` is not stated by `74700`, and `auto` is not
/// stated by `--auto-glossary`.
fn states_literal(text: &str, value: &str) -> bool {
    fn glued(byte: u8) -> bool {
        byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-'
    }
    let bytes = text.as_bytes();
    text.match_indices(value).any(|(at, _)| {
        let after = at + value.len();
        (at == 0 || !glued(bytes[at - 1])) && (after == bytes.len() || !glued(bytes[after]))
    })
}

/// The first fenced block after `heading` in `text`.
fn fenced_block_after(rel: &str, text: &str, heading: &str) -> String {
    let mut lines = text.lines();
    lines
        .find(|l| l.trim_end() == heading)
        .unwrap_or_else(|| panic!("{rel} should carry the heading `{heading}`"));
    let mut block = String::new();
    let mut inside = false;
    for line in lines {
        if line.trim_start().starts_with("```") {
            if inside {
                return block;
            }
            inside = true;
            continue;
        }
        if inside {
            block.push_str(line);
            block.push('\n');
        }
    }
    panic!("{rel} should carry a closed fenced block under `{heading}`");
}

fn shipped_flags() -> BTreeSet<String> {
    let mut flags = BTreeSet::new();
    for sub in SUBCOMMANDS {
        flags.extend(declared_flags(&help_text(sub)));
    }
    for own in CLAP_OWN {
        flags.remove(*own);
    }
    assert!(
        flags.contains("--input") && flags.contains("--rendered"),
        "the help scrape should find both subcommands' flags, got {flags:?}",
    );
    flags
}

fn shipped_defaults() -> BTreeMap<String, String> {
    let mut defaults: BTreeMap<String, String> = BTreeMap::new();
    for sub in SUBCOMMANDS {
        for (flag, value) in declared_defaults(&help_text(sub)) {
            if let Some(existing) = defaults.get(&flag) {
                assert_eq!(
                    existing, &value,
                    "`{flag}` defaults differently per subcommand; the usage \
                     blocks state one default per flag",
                );
            }
            defaults.insert(flag, value);
        }
    }
    for own in CLAP_OWN {
        defaults.remove(*own);
    }
    assert!(
        defaults.contains_key("--max-input-bytes") && defaults.contains_key("--port"),
        "the help scrape should find both subcommands' defaults, got {defaults:?}",
    );
    defaults
}

#[test]
fn every_shipped_flag_is_named_by_both_usage_blocks() {
    let shipped = shipped_flags();
    let mut missing = Vec::new();
    for (rel, heading) in GUARDED {
        let text = read(rel);
        let block = fenced_block_after(rel, &text, heading);
        let named = flags_named_in(&block);
        for flag in shipped.difference(&named) {
            missing.push(format!("{rel} (`{heading}`) never names `{flag}`"));
        }
    }
    assert!(
        missing.is_empty(),
        "`transync --help` accepts flags the published usage blocks do not \
         name. A block that silently lists a subset reads as the full set \
         (ti 57be8c) — add the flag, or say in the block that it is a \
         subset:\n  {}",
        missing.join("\n  "),
    );
}

#[test]
fn no_usage_block_names_a_flag_clap_does_not_accept() {
    let shipped = shipped_flags();
    let mut invented = Vec::new();
    for (rel, heading) in GUARDED {
        let text = read(rel);
        let block = fenced_block_after(rel, &text, heading);
        for flag in flags_named_in(&block).difference(&shipped) {
            invented.push(format!("{rel} (`{heading}`) names `{flag}`"));
        }
    }
    assert!(
        invented.is_empty(),
        "published usage block(s) name a flag the CLI does not accept — a \
         renamed or removed flag left behind in prose:\n  {}",
        invented.join("\n  "),
    );
}

#[test]
fn the_two_blocks_publish_the_same_flag_set() {
    let blocks: Vec<(&str, BTreeSet<String>)> = GUARDED
        .iter()
        .map(|(rel, heading)| {
            let text = read(rel);
            (
                *rel,
                flags_named_in(&fenced_block_after(rel, &text, heading)),
            )
        })
        .collect();
    let (first_rel, first) = &blocks[0];
    for (rel, other) in &blocks[1..] {
        assert_eq!(
            first, other,
            "the usage blocks in {first_rel} and {rel} publish different flag \
             sets; both claim to be the whole CLI surface",
        );
    }
}

#[test]
fn every_default_clap_prints_is_the_one_both_usage_blocks_state() {
    let defaults = shipped_defaults();
    let mut wrong = Vec::new();
    for (rel, heading) in GUARDED {
        let text = read(rel);
        let entries = flag_entries(&fenced_block_after(rel, &text, heading));
        for (flag, value) in &defaults {
            let Some((_, entry)) = entries.iter().find(|(named, _)| named.contains(flag)) else {
                wrong.push(format!(
                    "{rel} (`{heading}`) has no entry for `{flag}`, which clap \
                     defaults to `{value}`"
                ));
                continue;
            };
            if !states_literal(&stated_text(entry), value) {
                wrong.push(format!(
                    "{rel} (`{heading}`): the `{flag}` entry never states \
                     clap's default `{value}`"
                ));
            }
        }
    }
    assert!(
        wrong.is_empty(),
        "clap prints a default the published usage block(s) do not state. The \
         value is a literal, not prose: put clap's default in that flag's \
         entry, or change the default in `translate_cmd` / `serve_cmd` if the \
         document is the one that is right:\n  {}",
        wrong.join("\n  "),
    );
}

#[test]
fn default_scrapers_read_the_shapes_clap_and_the_blocks_are_written_in() {
    let help = "\
Options:
      --port <PORT>
          Port to bind.

          [default: 7470]

      --model <MODEL>
          Resolution order: this flag, else `TRANSYNC_OPENAI_MODEL`, else
          `gpt-5-chat-latest`

      --bind <BIND>  Address [default: \"127.0.0.1\"]
";
    let defaults = declared_defaults(help);
    assert_eq!(defaults.get("--port").map(String::as_str), Some("7470"));
    assert_eq!(
        defaults.get("--bind").map(String::as_str),
        Some("127.0.0.1"),
        "an inline, quoted default should read the same as a standalone one",
    );
    assert!(
        !defaults.contains_key("--model"),
        "a flag clap gives no default is not a subject; the literal in its \
         prose is resolved elsewhere",
    );

    let block = "\
transync translate
  [--max-input-bytes <n>]   (default: 67108864 = 64 MiB; refuse a larger --input
                            before parsing — exit 2.)
  --out-dir <dir>           (mutually exclusive with --output,
                            --map, and --html-out — see below)
  [--source-language <label>|auto] (default: \"auto\")
  [--auto-glossary]         (run the preflight)
";
    let entries = flag_entries(block);
    let named: Vec<Vec<String>> = entries
        .iter()
        .map(|(flags, _)| flags.iter().cloned().collect())
        .collect();
    assert_eq!(
        named,
        vec![
            vec!["--max-input-bytes".to_string()],
            vec!["--out-dir".to_string()],
            vec!["--source-language".to_string()],
            vec!["--auto-glossary".to_string()],
        ],
        "a wrapped description beginning with a flag must not open an entry, \
         and a flag named in an entry's prose must not key it",
    );
    let stated_for = |flag: &str| {
        entries
            .iter()
            .find(|(flags, _)| flags.contains(flag))
            .map(|(_, text)| stated_text(text))
            .unwrap_or_default()
    };
    assert!(states_literal(&stated_for("--max-input-bytes"), "67108864"));
    assert!(
        states_literal(&stated_for("--source-language"), "auto"),
        "the entry's own text is the scope — `auto` elsewhere in the block \
         must not answer for this flag",
    );
    assert!(
        !states_literal(&stated_for("--source-language"), "en"),
        "a default must be stated as a value, not found inside a word",
    );
    assert!(
        !states_literal(&stated_for("--auto-glossary"), "auto"),
        "`--auto-glossary` does not state the value `auto`",
    );
    assert!(
        !states_literal(&stated_text("[--source-language <label>|auto]"), "auto"),
        "a syntax sketch offers a value; it does not state a default",
    );
    assert!(!states_literal("port 74700", "7470"));
    assert!(states_literal("(default: 7470)", "7470"));
}

#[test]
fn scrapers_read_the_shapes_the_help_and_the_blocks_are_written_in() {
    let help = "\
Options:
      --input <INPUT>
          Refuse `--input` files larger than this. Does not affect
          `--profile` / `--system-prompt-file`.

      --target-direction <TARGET_DIRECTION>
          Possible values:
          - auto: resolve from the label
          - ltr:  force left-to-right

  -h, --help
          Print help
";
    let declared = declared_flags(help);
    assert!(declared.contains("--input"));
    assert!(declared.contains("--target-direction"));
    assert!(declared.contains("--help"));
    assert_eq!(declared.len(), 3, "descriptions must not declare flags");

    let block = "\
  --input <path>                 (required) source Markdown
  [--source-language <label>|auto]             default: auto
  [--max-units-per-batch <n>]    hard unit cap per batch; n >= 1
  [--quiet|--verbose]
  see \"--out-dir semantics\" below — em-dash prose
";
    let named = flags_named_in(block);
    assert_eq!(
        named,
        [
            "--input",
            "--max-units-per-batch",
            "--out-dir",
            "--quiet",
            "--source-language",
            "--verbose",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect::<BTreeSet<String>>(),
    );

    // A shorter flag is not found inside a longer one.
    let longer = flags_named_in("[--output-expansion-factor <f>]");
    assert_eq!(longer.len(), 1);
    assert!(longer.contains("--output-expansion-factor"));
    assert!(!longer.contains("--output"));

    let doc = "## H\n\ntext\n\n```\n  --input <path>\n```\n\nafter\n";
    assert_eq!(
        fenced_block_after("t.md", doc, "## H"),
        "  --input <path>\n"
    );
}
