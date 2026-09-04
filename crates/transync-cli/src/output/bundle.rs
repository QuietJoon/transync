//! The `--html-out` bundle's rendered shell: the assets embedded at compile
//! time, the `index.html` template filler, and the attribute escaping the
//! substituted values go through.
//!
//! Lifted out of `output.rs` unchanged by OI-0043.
//!
//! TRACE: ADR-0006
//! TRACE: SCN-13

use std::path::Path;

/// The embedded JS sync engine source. Built into the CLI binary at compile
/// time so `transync translate --html-out <dir>` can write a self-contained
/// demo bundle without consulting the workspace.
///
/// TRACE: SCN-13
const SYNC_JS: &str = include_str!("../../web/sync.js");

/// The embedded `index.html` template body.
///
/// TRACE: ADR-0006
const INDEX_TPL: &str = include_str!("../../web/index.html.tpl");

/// The embedded DOMPurify build (vendored; mirror of
/// `web/vendor/purify.min.js` — byte-equality enforced by
/// `tests/sync_js_drift.rs`). The demo shell sanitizes fetched
/// fragments before mounting and fails closed without it. OI-0001.
const PURIFY_JS: &str = include_str!("../../web/purify.min.js");

/// What `--strict-csp` puts in the bundle shell's `<head>`: a whole
/// `Content-Security-Policy` `<meta>` element, preceded by the newline and
/// indent that place it directly under the charset declaration. The flag is
/// off by default, the alternative substitution is the empty string, and the
/// slot sits at the END of the charset line — so a run without the flag
/// produces a byte-identical `index.html` to a pre-flag one (OI-0018, posture
/// amended 2026-08-07).
///
/// Every directive is derived from what this bundle actually loads:
///
/// * `default-src 'self'` — the fallback for everything not named below
///   (fonts, media, frames, workers): same origin or nothing. `'self'` is
///   origin-scoped, not directory-scoped — it stops remote hosts, not sibling
///   paths under whatever document root the bundle is served from.
/// * `img-src 'self' data:` — the point of the flag. Rendered content carries
///   whatever image URLs the untrusted source Markdown named, and comrak's
///   link sanitizer passes `data:image/*` through, so inline images keep
///   rendering while a remote host never learns that a viewer opened the
///   document (the tracking-pixel exposure).
/// * `script-src 'self' 'unsafe-inline'` — `purify.min.js` and `sync.js` are
///   bundle files, and `index.html`'s module script is inline. No nonce or
///   hash is emitted, so `'unsafe-inline'` is honored rather than ignored.
/// * `style-src 'self' 'unsafe-inline'` — the entire theme sheet is the inline
///   `<style>` block, and sanitized content may carry `style` attributes.
/// * `connect-src 'self'` — the shell fetches `source.html`, `target.html` and
///   `alignment.json` from beside itself, and nothing else.
/// * `object-src 'none'` / `base-uri 'none'` — the bundle has no plugin
///   content and no `<base>`; both would otherwise be reachable through
///   rendered content.
///
/// `frame-ancestors`, `sandbox` and `report-uri` are deliberately absent: a
/// `<meta>`-delivered policy ignores them, so naming them would only add a
/// console warning. The bundle is meant to be *served* (its fetches already
/// fail under `file://`), which is also where `'self'` has a meaning.
///
/// TRACE: OI-0018
const CSP_META: &str = concat!(
    "\n    <meta http-equiv=\"Content-Security-Policy\" content=\"",
    "default-src 'self'; ",
    "img-src 'self' data:; ",
    "script-src 'self' 'unsafe-inline'; ",
    "style-src 'self' 'unsafe-inline'; ",
    "connect-src 'self'; ",
    "object-src 'none'; ",
    "base-uri 'none'",
    "\">",
);

/// Assemble the six demo-bundle payloads (ADR-0006 + OI-0001) without
/// writing anything. The caller commits them through
/// [`super::write_fileset_atomic`] alongside the other CLI outputs.
///
/// `index.html.tpl`'s placeholders are substituted: `{{TRANSYNC_DOC_LANG}}`
/// becomes the target language (the document's primary content language),
/// the pane `{{TRANSYNC_*_LANG_ATTR}}` slots stamp per-pane `lang`
/// attributes for screen readers and hyphenation (R0008-0050), the pane
/// `{{TRANSYNC_*_DIR_ATTR}}` slots stamp per-pane text direction (OI-0032),
/// and `{{TRANSYNC_TITLE}}` becomes `title` — the caller's already-resolved
/// document title (ti 0f26b5; the resolution order lives in
/// [`crate::translate_cmd`]'s `resolve_bundle_title`, not here).
/// `source_language` is the caller's resolved source label (the detected
/// language when the run used `auto`), or empty when unknown.
///
/// `title` is the one slot whose value can come from the **source document**
/// rather than from an argument, which is why two things about the
/// substitution are deliberate: the value is escaped like every other caller
/// string, and the fill is [`fill_template`]'s single pass rather than a chain
/// of `String::replace` (untrusted text that looks like a placeholder must not
/// become one).
///
/// The two `*_dir_attr` arguments come from
/// [`crate::direction::dir_attr`] — either the fixed literal ` dir="rtl"`
/// or the empty string, never caller text — so unlike the language labels
/// they need no attribute escaping. `<html>` deliberately carries only
/// `lang`: stamping `dir` there would flip the themer/legend chrome, and
/// the panes are the content.
///
/// `strict_csp` (`--strict-csp`, OI-0018) selects between the two fixed
/// [`CSP_META`] substitutions rather than passing text in, so the flag cannot
/// become a `<head>` injection point. It is a `bool` and not a ninth `&str`
/// slot for the same reason the others are strings: a caller cannot silently
/// transpose it with one of them.
///
/// TRACE: ADR-0006
/// TRACE: SCN-12
// One flat parameter per template slot: the placeholders are independent
// strings resolved by different rules, and a wrapper struct would only move
// the same list one indirection away.
#[allow(clippy::too_many_arguments)]
pub fn html_bundle_files(
    dir: &Path,
    source_fragment: &str,
    target_fragment: &str,
    alignment_json: &[u8],
    title: &str,
    source_language: &str,
    target_language: &str,
    source_dir_attr: &str,
    target_dir_attr: &str,
    strict_csp: bool,
) -> Vec<(std::path::PathBuf, Vec<u8>)> {
    let index = fill_template(
        INDEX_TPL,
        &[
            (
                "{{TRANSYNC_DOC_LANG}}",
                escape_attr(target_language.trim()).as_str(),
            ),
            ("{{TRANSYNC_TITLE}}", escape_attr(title.trim()).as_str()),
            (
                "{{TRANSYNC_CSP_META}}",
                if strict_csp { CSP_META } else { "" },
            ),
            (
                "{{TRANSYNC_SOURCE_LANG_ATTR}}",
                lang_attr(source_language).as_str(),
            ),
            (
                "{{TRANSYNC_TARGET_LANG_ATTR}}",
                lang_attr(target_language).as_str(),
            ),
            ("{{TRANSYNC_SOURCE_DIR_ATTR}}", source_dir_attr),
            ("{{TRANSYNC_TARGET_DIR_ATTR}}", target_dir_attr),
        ],
    );
    vec![
        (dir.join("index.html"), index.into_bytes()),
        (dir.join("source.html"), source_fragment.as_bytes().to_vec()),
        (dir.join("target.html"), target_fragment.as_bytes().to_vec()),
        (dir.join("alignment.json"), alignment_json.to_vec()),
        (dir.join("sync.js"), SYNC_JS.as_bytes().to_vec()),
        (dir.join("purify.min.js"), PURIFY_JS.as_bytes().to_vec()),
    ]
}

/// Fill the shell template's `{{TRANSYNC_*}}` slots in **one pass**: scan the
/// template once, and push each matched slot's value straight to the output
/// where nothing looks at it again. An unknown `{{…}}` run is copied verbatim.
///
/// The single pass is the point. Chained `String::replace` calls rescan text
/// they have already substituted, so one slot's value can be re-substituted by
/// a later slot — and since ti 0f26b5 one of these values is untrusted
/// document content: the title defaults to the source document's first H1
/// (invariant 7). Under a replace chain, a document whose heading reads
/// `{{TRANSYNC_CSP_META}}` would pull a `<meta>` element into the shell's
/// `<title>`, and a heading reading `{{TRANSYNC_TARGET_LANG_ATTR}}` would pull
/// in an attribute fragment. Both are inert where they land (`<title>` is
/// RCDATA, and the values are fixed literals or escaped labels), so this is
/// hardening rather than a fix for a live escape — but "a substituted value is
/// never rescanned" is a structural property, while "the replaces happen to be
/// ordered so it does not matter" is one a later edit silently revokes.
fn fill_template(tpl: &str, slots: &[(&str, &str)]) -> String {
    let mut out = String::with_capacity(tpl.len());
    let mut rest = tpl;
    while let Some(open) = rest.find("{{") {
        out.push_str(&rest[..open]);
        rest = &rest[open..];
        let Some(close) = rest.find("}}") else {
            // An unterminated `{{` is template text, not a slot. `rest` now
            // starts at it, and the push below emits it once.
            break;
        };
        let (name, tail) = rest.split_at(close + "}}".len());
        match slots.iter().find(|(slot, _)| *slot == name) {
            Some((_, value)) => out.push_str(value),
            None => out.push_str(name),
        }
        rest = tail;
    }
    out.push_str(rest);
    out
}

/// Escape a string for use inside a double-quoted HTML attribute value.
/// The language labels are caller-supplied, so escaping keeps a stray `"`
/// (or markup) from breaking out of the `lang="…"` attribute in the
/// generated `index.html`.
///
/// The `<title>` text node uses the same escaper (ti 0f26b5). Escaping `&`
/// and `<` is all a text node strictly needs, and this escapes those plus
/// three characters a text node would render identically either way (`>`,
/// `"`, `'` come back as themselves when the browser decodes the entity) — a
/// superset, not a mismatch. One escaper for both is deliberate: the title is
/// the one slot fed by untrusted source content, and a second escaper is a
/// second thing that can be wrong.
fn escape_attr(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            other => out.push(other),
        }
    }
    out
}

/// Render a leading-space ` lang="…"` attribute for a pane, or an empty
/// string when the language is unknown (e.g. `--source-language auto` with
/// no detection). R0008-0050.
fn lang_attr(language: &str) -> String {
    let lang = language.trim();
    if lang.is_empty() {
        String::new()
    } else {
        format!(" lang=\"{}\"", escape_attr(lang))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The bundle's `index.html` for one `--strict-csp` setting, with every
    /// other slot held fixed so the two shells differ by the flag alone.
    fn bundle_index(strict_csp: bool) -> String {
        titled_bundle_index("transync", strict_csp)
    }

    /// The bundle's `index.html` for one title, with every other slot fixed.
    fn titled_bundle_index(title: &str, strict_csp: bool) -> String {
        let files = html_bundle_files(
            Path::new("html"),
            "<main></main>",
            "<main></main>",
            b"{}",
            title,
            "en",
            "ko",
            "",
            "",
            strict_csp,
        );
        let (_, index) = files
            .into_iter()
            .find(|(path, _)| path.ends_with("index.html"))
            .expect("the bundle carries an index.html");
        String::from_utf8(index).expect("the shell is UTF-8")
    }

    /// The text between `<title>` and `</title>` in a shell.
    fn title_of(index: &str) -> &str {
        let open = index
            .find("<title>")
            .expect("the shell has a title element")
            + "<title>".len();
        let close = index[open..]
            .find("</title>")
            .expect("the title element closes");
        &index[open..open + close]
    }

    /// OI-0018 (posture amended 2026-08-07): the CSP is opt-in, and opting out
    /// leaves the shell exactly as it was before the flag existed. Asserting
    /// "the strict shell minus the element IS the default shell" pins both
    /// halves at once — the element is added, and nothing else moved.
    #[test]
    fn the_csp_element_is_the_only_difference_the_flag_makes() {
        let plain = bundle_index(false);
        let strict = bundle_index(true);

        assert!(
            !plain.contains("Content-Security-Policy"),
            "the default posture emits no policy: {plain}"
        );
        assert!(
            strict.contains(CSP_META),
            "--strict-csp must emit the policy element verbatim: {strict}"
        );
        assert_eq!(
            strict.replace(CSP_META, ""),
            plain,
            "--strict-csp must add the policy element and change nothing else"
        );
    }

    /// The policy has to sit in `<head>` (a `<meta>` policy governs only what
    /// is parsed after it), and it has to carry every directive the bundle
    /// needs in order to still work: its own inline style block and module
    /// script, its two sibling scripts, its three same-origin fetches, and
    /// `data:` images. The template also has to leave no slot unsubstituted in
    /// either mode.
    #[test]
    fn the_strict_shell_is_still_a_working_bundle_shell() {
        let strict = bundle_index(true);
        let head_end = strict.find("</head>").expect("the shell has a head");
        let policy_at = strict
            .find("http-equiv=\"Content-Security-Policy\"")
            .expect("the strict shell declares a policy");
        assert!(
            policy_at < head_end,
            "a meta policy must be declared in <head>, before what it governs"
        );

        for directive in [
            "default-src 'self'",
            "img-src 'self' data:",
            "script-src 'self' 'unsafe-inline'",
            "style-src 'self' 'unsafe-inline'",
            "connect-src 'self'",
            "object-src 'none'",
            "base-uri 'none'",
        ] {
            assert!(
                strict.contains(directive),
                "the policy must carry `{directive}` or the bundle stops working: {strict}"
            );
        }

        for shell in [strict, bundle_index(false)] {
            assert!(
                !shell.contains("{{TRANSYNC_"),
                "every template slot must be substituted: {shell}"
            );
        }
    }

    /// ti 0f26b5: the caller's title reaches `<title>` verbatim, and the two
    /// language slots are still the two the shell needs — `<html lang>` is the
    /// target (the document's own language) while the panes carry one each.
    #[test]
    fn the_shell_carries_the_callers_title_and_the_document_language() {
        let index = titled_bundle_index("Design Notes", false);
        assert_eq!(title_of(&index), "Design Notes");
        assert!(
            index.contains("<html lang=\"ko\">"),
            "the document language is the TARGET language: {index}"
        );
    }

    /// ti 0f26b5 / invariant 7: the title can come from the source document's
    /// first H1, so it is untrusted text. It must land in `<title>` as text —
    /// escaped, never as markup — and a title that *looks* like a template
    /// slot must stay text too: the single-pass fill never rescans a value it
    /// has already substituted.
    #[test]
    fn an_untrusted_title_is_escaped_and_never_re_substituted() {
        let hostile = titled_bundle_index("</title><script>alert(1)</script> & \"co\"", false);
        assert_eq!(
            title_of(&hostile),
            "&lt;/title&gt;&lt;script&gt;alert(1)&lt;/script&gt; &amp; &quot;co&quot;",
            "every markup character must be escaped inside the title text"
        );
        assert!(
            !hostile.contains("<script>"),
            "a heading must not be able to open an element: {hostile}"
        );

        let slotlike = titled_bundle_index("{{TRANSYNC_CSP_META}}", true);
        assert_eq!(
            title_of(&slotlike),
            "{{TRANSYNC_CSP_META}}",
            "a substituted value must never be substituted into again"
        );
        assert_eq!(
            slotlike.matches("Content-Security-Policy").count(),
            1,
            "the policy element belongs in <head> once, not also in the title: {slotlike}"
        );
    }

    /// The filler substitutes what it knows and leaves everything else alone —
    /// including a `{{…}}` run that names no slot and an unterminated `{{`,
    /// both of which are template text rather than a slot with a missing
    /// value.
    #[test]
    fn the_filler_replaces_known_slots_and_copies_the_rest() {
        let slots = [("{{A}}", "1"), ("{{B}}", "")];
        assert_eq!(fill_template("x{{A}}y{{B}}z", &slots), "x1yz");
        assert_eq!(fill_template("{{A}}{{A}}", &slots), "11");
        assert_eq!(
            fill_template("{{UNKNOWN}} {{A}}", &slots),
            "{{UNKNOWN}} 1",
            "an unknown slot is template text, not an empty substitution"
        );
        assert_eq!(fill_template("tail {{A", &slots), "tail {{A");
        assert_eq!(fill_template("no slots", &slots), "no slots");
    }
}
