//! Capped file reads and profile resolution for `transync translate`.
//!
//! Every byte the command reads enters here: the `--input` document under
//! `--max-input-bytes`, and the auxiliary `--profile` / `--system-prompt-file`
//! texts under the fixed [`AUX_READ_LIMIT`]. The error types keep the CLI's
//! exit-code split visible to the caller without deciding the code here.
//!
//! TRACE: EXT-2026-07 P1-7
//! TRACE: contracts.md §2

use std::io::Read;
use std::path::Path;
use transync::profile::{ProfileMetadata, default_profile, load_profile};

/// Fixed read cap for auxiliary text inputs (`--profile` TOML,
/// `--system-prompt-file`): 4 MiB. Not affected by `--max-input-bytes`.
/// EXT-2026-07 P1-7.
const AUX_READ_LIMIT: u64 = 4 * 1024 * 1024;

/// Failure modes of [`read_capped`]. Structured so callers can map a size-cap
/// breach to exit 2 (InputReadFailure) while an ordinary unreadable file
/// keeps its usual exit code.
///
/// TRACE: EXT-2026-07 P1-7
pub(crate) enum CappedReadError {
    Io(std::io::Error),
    TooLarge { limit: u64 },
    NotUtf8,
}

/// Read a whole file as UTF-8, refusing anything larger than `limit` bytes.
///
/// EXT-2026-07 P1-7: reads `limit + 1` bytes through `Read::take` rather than
/// trusting `metadata().len()`, so a racing, lying, or still-growing file
/// cannot slip past the admission check — the extra byte is the signal that
/// the file is over the cap.
pub(crate) fn read_capped(path: &Path, limit: u64) -> Result<String, CappedReadError> {
    let file = std::fs::File::open(path).map_err(CappedReadError::Io)?;
    let mut buf = Vec::new();
    file.take(limit.saturating_add(1))
        .read_to_end(&mut buf)
        .map_err(CappedReadError::Io)?;
    if buf.len() as u64 > limit {
        return Err(CappedReadError::TooLarge { limit });
    }
    String::from_utf8(buf).map_err(|_| CappedReadError::NotUtf8)
}

/// How much of a document's head the HTML-document sniff reads: 1 KiB.
///
/// The check is a *preamble* check by decision (ticket `13e145` puts
/// content-based sniffing out of scope), and an HTML document declares itself
/// in its first line — `<!DOCTYPE html>` and `<html …>` are what a browser's
/// own sniff keys on. The window is what keeps the cost of the check
/// independent of the 64 MiB an `--input` may legitimately be: the scan skips
/// leading blanks and comments, and without a bound a megabyte of leading
/// whitespace would be a megabyte of scanning.
const HTML_SNIFF_WINDOW: usize = 1024;

/// The HTML-document marker `source`'s preamble opens with, if any.
///
/// Returns the matched marker (`<!doctype` or `<html`) so the caller's
/// diagnostic can quote what it actually saw rather than assert a category.
///
/// **Why the CLI asks this at all.** `transync translate` accepts GFM
/// Markdown. Handed an HTML document it does not fail — it produces output
/// that is close enough to correct to be believed: runs that open with a tag
/// become `BlockKind::Html` units and splice back byte-exact, while every
/// blank-line-separated run that does *not* open with a tag re-enters as
/// **Markdown**. What that costs was measured against the echo stub rather
/// than reasoned about (ti `d990b6`):
///
/// - Prose is re-read under Markdown inline rules. `*asterisks*` and
///   `_underscores_` are consumed as emphasis delimiters, `[` opens a link,
///   and `&amp;` is decoded — so the provider is handed Markdown source, and
///   what it returns is re-rendered by those same rules.
/// - Sections and the document title come from Markdown headings, so an
///   `<h1>` leaves both empty: every unit's `section_path` is `[]`, the title
///   the provider is told is `None`, and the HTML bundle falls through to the
///   literal `transync`. Nothing says so.
/// - A four-space-indented run becomes an indented code block, which the
///   engine translates and re-emits **fenced** (ti `457e51`). The bytes are
///   translated, but the document's shape is not the one that went in.
///
/// All three are silent, and the run still exits 0 with its artifact named a
/// translated document, which is why the boundary has to decline the input
/// rather than trust the report. The third one used to be the exception — the
/// *loud* `fallback_source` this path could point at — and ti `457e51`, by
/// making indented code translate, removed the last thing here that reported
/// anything at all.
///
/// **What it matches.** Blank lines, a UTF-8 BOM, and any run of leading HTML
/// comments are skipped first — a real page often opens with a license banner,
/// and a Markdown file that opens with a comment is not made HTML by what
/// follows it. What remains must *begin* with `<!doctype` or `<html`,
/// ASCII-case-insensitively, followed by whitespace, `>`, or the end of the
/// window; `<htmlish>` is not a match. Both markers are declarations a Markdown
/// document has no reason to open with, which is what makes the check cheap to
/// be sure about — and the reason the answer is overridable rather than final.
///
/// **Indentation is read the way CommonMark reads it**, which is why the skip
/// is [`skip_blank_lines_and_html_block_indent`] and not `str::trim_start`.
/// Up to three spaces still open an HTML block; a fourth makes the line an
/// **indented code block**, so a Markdown document whose first block is a
/// four-space-indented sample of HTML is not an HTML document and must not be
/// refused as one. Blindly trimming all leading whitespace turned exactly that
/// document — the one the refusal message itself describes — into a false
/// positive (found in adversarial review of ti `13e145`).
///
/// TRACE: ti 13e145
pub(crate) fn html_document_marker(source: &str) -> Option<&'static str> {
    let mut head = source;
    if head.len() > HTML_SNIFF_WINDOW {
        let mut cut = HTML_SNIFF_WINDOW;
        while cut > 0 && !head.is_char_boundary(cut) {
            cut -= 1;
        }
        head = &head[..cut];
    }

    let mut rest = skip_blank_lines_and_html_block_indent(head.trim_start_matches('\u{feff}'));
    // Leading comments, as many as the window holds. An unterminated one ends
    // the skip: its content is not preamble this check can see past.
    while let Some(after) = rest.strip_prefix("<!--") {
        let Some(end) = after.find("-->") else { break };
        rest = skip_blank_lines_and_html_block_indent(&after[end + "-->".len()..]);
    }

    let bytes = rest.as_bytes();
    for marker in ["<!doctype", "<html"] {
        let m = marker.as_bytes();
        if bytes.len() < m.len() || !bytes[..m.len()].eq_ignore_ascii_case(m) {
            continue;
        }
        match bytes.get(m.len()) {
            None | Some(b'>') => return Some(marker),
            Some(c) if c.is_ascii_whitespace() => return Some(marker),
            _ => {}
        }
    }
    None
}

/// Drop leading blank lines, then the leading indentation CommonMark still
/// lets an HTML block carry (up to three spaces).
///
/// A fourth space — or a tab, which counts as an indent of four — opens an
/// **indented code block** instead, so the line is Markdown content and the
/// caller must not read a marker out of it. On that input the indentation is
/// left in place, which is what makes the marker comparison fail.
///
/// TRACE: ti 13e145
fn skip_blank_lines_and_html_block_indent(source: &str) -> &str {
    let mut rest = source;
    // Blank lines carry no content, so they cannot be the indent of anything.
    // Only a line that is followed by more input can be skipped whole: a
    // trailing run of spaces at end of input is not a blank *line*.
    while let Some(nl) = rest.find('\n') {
        if !rest[..nl].trim().is_empty() {
            break;
        }
        rest = &rest[nl + 1..];
    }
    let indent = rest.len() - rest.trim_start_matches(' ').len();
    if indent <= 3 { &rest[indent..] } else { rest }
}

/// Error from [`resolve_profile`], carrying the CLI exit-code split: an
/// oversized `--profile` / `--system-prompt-file` is an InputReadFailure
/// (exit 2); every other problem (bad TOML, unreadable file, non-UTF-8) is
/// an argument error (exit 1).
///
/// TRACE: EXT-2026-07 P1-7
pub(crate) enum ProfileError {
    Arg(String),
    ReadTooLarge(String),
}

/// Map a capped-read failure on an auxiliary input to a [`ProfileError`],
/// preserving the exit-code split (size cap → exit 2, else exit 1).
///
/// TRACE: EXT-2026-07 P1-7
fn aux_read_error(err: CappedReadError, flag: &str, path: &Path) -> ProfileError {
    match err {
        CappedReadError::TooLarge { limit } => ProfileError::ReadTooLarge(format!(
            "{flag} {} exceeds the {limit}-byte read limit",
            path.display()
        )),
        CappedReadError::Io(e) => {
            ProfileError::Arg(format!("failed to read {flag} {}: {e}", path.display()))
        }
        CappedReadError::NotUtf8 => {
            ProfileError::Arg(format!("{flag} {} is not valid UTF-8", path.display()))
        }
    }
}

/// Resolve the active profile, layering CLI overrides on top of either a
/// `--profile` TOML file or the embedded default. `--system-prompt` /
/// `--system-prompt-file` replace the loaded profile's `prompt_body`
/// template; template variables are substituted at batch-build time.
/// `--profile` / `--system-prompt-file` reads are capped at
/// [`AUX_READ_LIMIT`] (EXT-2026-07 P1-7).
///
/// TRACE: SCN-09
/// TRACE: contracts.md §2
pub(crate) fn resolve_profile(
    profile_path: Option<&Path>,
    system_prompt: Option<&str>,
    system_prompt_file: Option<&Path>,
) -> Result<ProfileMetadata, ProfileError> {
    let mut profile = match profile_path {
        Some(path) => {
            let toml_text = read_capped(path, AUX_READ_LIMIT)
                .map_err(|e| aux_read_error(e, "--profile", path))?;
            load_profile(&toml_text)
                .map_err(|e| ProfileError::Arg(format!("invalid profile TOML: {e}")))?
        }
        None => default_profile(),
    };

    if let Some(text) = system_prompt {
        profile.prompt_body = text.to_string();
    } else if let Some(path) = system_prompt_file {
        let text = read_capped(path, AUX_READ_LIMIT)
            .map_err(|e| aux_read_error(e, "--system-prompt-file", path))?;
        profile.prompt_body = text;
    }

    Ok(profile)
}

/// The preamble sniff, both directions: what an HTML document looks like, and
/// what a Markdown document carrying HTML looks like. The second half is the
/// load-bearing one — this check gates a refusal, and a false positive stops
/// a run that would have been correct (ti `13e145`).
#[cfg(test)]
mod html_sniff_tests {
    use super::html_document_marker;

    #[test]
    fn html_documents_are_recognized() {
        for src in [
            "<!DOCTYPE html>\n<html>\n<body><p>hi</p></body>\n</html>\n",
            "<!doctype html>\n",
            "\u{feff}\n\n  <!DOCTYPE HTML PUBLIC \"-//W3C//DTD HTML 4.01//EN\">\n",
            "<html lang=\"en\">\n",
            "<HTML>\n",
            // A license banner ahead of the declaration is still a preamble.
            "<!-- Copyright 2026 -->\n<!-- generated -->\n<!doctype html>\n",
        ] {
            assert!(
                html_document_marker(src).is_some(),
                "should sniff as an HTML document: {src:?}",
            );
        }
    }

    #[test]
    fn markdown_carrying_html_is_not_an_html_document() {
        for src in [
            "# Title\n\n<div align=\"center\">hero</div>\n",
            // The islands ADR-0018 exists for, with nothing but HTML above
            // the fold.
            "<div align=\"center\">\n<img src=\"logo.png\">\n</div>\n\nProse.\n",
            "<details>\n<summary>Click</summary>\n\nBody.\n\n</details>\n",
            "<!-- a note -->\n\nOrdinary paragraph.\n",
            // Prefix-only lookalikes: the marker must END the token.
            "<htmlish>\n",
            "<doctype>\n",
            "",
            "\n\n\n",
        ] {
            assert_eq!(
                html_document_marker(src),
                None,
                "must not sniff as an HTML document: {src:?}",
            );
        }
    }

    /// Four spaces of indentation make the line an **indented code block**,
    /// not an HTML block — so a Markdown document whose first block is a code
    /// sample of HTML is not an HTML document. Up to three spaces still open
    /// an HTML block and must still be sniffed. Found in adversarial review:
    /// `trim_start` erased the one piece of syntax that separates the two, and
    /// `transync translate --input a.md` on such a file exited 2.
    #[test]
    fn an_indented_code_sample_of_html_is_not_an_html_document() {
        for src in [
            "    <html>\n    <body>hello</body>\n\nProse.\n",
            "    <!DOCTYPE html>\n\nProse.\n",
            "\n\n    <html>\n",
            // A tab is an indent of four, so it opens a code block too.
            "\t<html>\n",
            // The comment skip must read the line after it the same way.
            "<!-- sample -->\n    <html>\n",
        ] {
            assert_eq!(
                html_document_marker(src),
                None,
                "an indented code block is Markdown content: {src:?}",
            );
        }
        // The other side of the same boundary: three spaces is still HTML.
        for src in [
            "   <html>\n",
            "\n   <!doctype html>\n",
            "<!-- c -->\n   <html>\n",
        ] {
            assert!(
                html_document_marker(src).is_some(),
                "three spaces still open an HTML block: {src:?}",
            );
        }
    }

    /// The window bounds the scan, so a declaration pushed past it is not
    /// found — a deliberate limit of a preamble check, pinned so a later
    /// reader does not mistake it for a bug.
    #[test]
    fn a_declaration_past_the_window_is_out_of_reach() {
        let src = format!("{}<!doctype html>\n", " ".repeat(4096));
        assert_eq!(html_document_marker(&src), None);
    }

    /// The truncation is char-boundary safe: a multi-byte character straddling
    /// the window edge must not panic the slice.
    #[test]
    fn the_window_never_splits_a_character() {
        let src = "가".repeat(2048);
        assert_eq!(html_document_marker(&src), None);
    }
}
