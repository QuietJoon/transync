//! The token-stream pin (spec 2026-08-20 §5).
//!
//! Wave 0 gives `TagToken::Open` a `span` and adds a `TagToken::Skip`
//! variant. Both are meant to be **inert** for the two consumers that ship
//! today — `tag_inventory` (validation layer 3) and `balance_fragment` (the
//! render path). "Meant to be" is not evidence, so this file is the evidence:
//! the `Open`/`Close` projection of `scan_tags`, `tag_inventory`'s output and
//! `balance_fragment`'s output are frozen against goldens generated from the
//! code as it stood BEFORE the token change, over the repository's existing
//! HTML-bearing fixtures.
//!
//! The goldens are regenerated only by running the `#[ignore]`d
//! `regenerate_goldens` below deliberately. **If the pin goes red, the token
//! change was not inert — do not re-bless the golden, find out which region
//! moved.**
//!
//! TRACE: ti 490d97 wave 0
//! TRACE: DCR-0032

use std::path::{Path, PathBuf};

use transync_html::{TagToken, balance_fragment, scan_tags, tag_inventory};

/// Repo root, derived from this crate's manifest dir
/// (`crates/transync-html` -> `../..`), the idiom `docs_index_drift.rs` uses.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root should resolve from CARGO_MANIFEST_DIR/../..")
}

fn goldens_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/goldens")
}

/// The repository's existing HTML-bearing fixtures. They are Markdown
/// documents; `scan_tags` is a byte scanner and does not care.
const FIXTURES: &[(&str, &str)] = &[
    (
        "scn-15-html-blocks",
        "crates/transync/tests/fixtures/scn-15-html-blocks.md",
    ),
    (
        "scn-14-full",
        "crates/transync/tests/fixtures/scn-14-full.md",
    ),
    (
        "reader-honesty",
        "crates/transync/tests/fixtures/reader-honesty.md",
    ),
];

/// The regions the R0002-* / R0003-* incidents were about, plus the two
/// doctype spellings. Every one tokenizes the same way before and after
/// wave 0, which is what makes this golden a valid before/after comparison.
///
/// Two of them — `doctype-upper` and `doctype-lower` — *are* bare `<!…>`
/// regions, i.e. exactly the region where wave 0 changes tokenization. They
/// still cannot move this projection, because their interiors hold no
/// tag-shaped bytes: before wave 0 the scanner walked past them emitting
/// nothing (`!` fails the tag-open state's ASCII-letter test), after it they
/// emit one `Skip`, and the projection below drops both. What the corpus
/// deliberately does **not** contain is a bare `<!…>` whose interior *does*
/// hold tag-shaped bytes — `<! <div> >`, where the `<div>` used to tokenize
/// as markup and no longer does. That is the one real behaviour change in
/// the wave, so it is pinned by its own in-crate test rather than here
/// (`tag_shaped_bytes_inside_a_bogus_comment_are_not_markup`).
///
/// The `<![CDATA[…]]>` entries *do* hold tag-shaped bytes, and that is fine:
/// the CDATA branch, with both terminator modes, shipped with R0003-0067
/// long before this wave, so those regions were already skipped and their
/// tokenization does not move either.
const EDGE_CASES: &[(&str, &str)] = &[
    (
        "cdata-foreign",
        "<svg><text><![CDATA[<b>bold</b>]]></text></svg>",
    ),
    ("cdata-html", "<p><![CDATA[</b>]]></p>"),
    (
        "rcdata-textarea",
        "<textarea>Use </p> to close a paragraph</textarea>",
    ),
    ("rcdata-title", "<title>a < b </i> c</title>"),
    (
        "script-raw-text",
        "<script>if (a < b) { s = \"</div>\"; }</script>",
    ),
    ("digit-tag", "Rows <2026 total> here"),
    ("colon-name", "<div:x>y"),
    ("unquoted-slash", "<a href=https://example.com/>Example</a>"),
    ("optional-end-tags", "<ul><li>a<li>b"),
    ("comment", "<div><!-- hidden -->visible</div>"),
    ("doctype-upper", "<!DOCTYPE html>\n<p>after</p>"),
    ("doctype-lower", "<!doctype html>\n<p>after</p>"),
];

/// Every corpus entry as `(name, source)`, fixtures first.
fn corpus() -> Vec<(String, String)> {
    let root = repo_root();
    let mut out = Vec::new();
    for (name, rel) in FIXTURES {
        let path = root.join(rel);
        let src = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("{rel} should be readable ({}): {e}", path.display()));
        out.push(((*name).to_string(), src));
    }
    for (name, src) in EDGE_CASES {
        out.push(((*name).to_string(), (*src).to_string()));
    }
    out
}

/// The stream every shipped consumer sees: `Open` and `Close`, nothing else.
///
/// Written as an `if let` chain, not a `match`, on purpose. This projection's
/// contract is "the two shapes that existed before wave 0", so a third
/// variant must fall outside it **without an edit to this file** — otherwise
/// the pin's own source would move in the same commit as the change it
/// exists to police. `Open.span` is deliberately unread here for the same
/// reason; Task 4's `an_open_tag_span_slices_back_to_its_own_bytes` covers it.
fn stream(html: &str) -> Vec<String> {
    let mut out = Vec::new();
    for token in scan_tags(html) {
        if let TagToken::Open {
            name, self_closing, ..
        } = &token
        {
            out.push(format!("O:{name}:{self_closing}"));
        } else if let TagToken::Close { name, span, .. } = &token {
            out.push(format!("C:{name}@{}..{}", span.0, span.1));
        }
    }
    out
}

/// The token golden, rendered from the corpus.
fn render_token_golden() -> String {
    let mut out = String::new();
    for (name, src) in corpus() {
        out.push_str(&format!("### {name}\n"));
        out.push_str(&format!("inventory: {}\n", tag_inventory(&src).join("|")));
        out.push_str(&format!("stream: {}\n", stream(&src).join("|")));
    }
    out
}

#[test]
fn the_open_close_stream_and_tag_inventory_are_byte_identical_to_the_golden() {
    let path = goldens_dir().join("token-stream.txt");
    let expected = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "{} should be readable: {e}. Generate it with: cargo test -p \
             transync-html --test token_stream_pin -- --ignored --test-threads=4",
            path.display()
        )
    });

    // Anti-vacuity, both directions: a golden that lost its sections, or one
    // that never recorded a token, would make the compare pass for the wrong
    // reason.
    assert_eq!(
        expected.lines().filter(|l| l.starts_with("### ")).count(),
        FIXTURES.len() + EDGE_CASES.len(),
        "the golden does not cover the whole corpus — it was generated \
         against a different FIXTURES/EDGE_CASES set"
    );
    assert!(
        expected.lines().any(|l| l.starts_with("stream: O:")),
        "the golden records no open tag at all — the projection stopped \
         seeing tokens"
    );

    assert_eq!(
        render_token_golden(),
        expected,
        "the Open/Close stream or tag_inventory moved. Wave 0's token changes \
         (Open.span, TagToken::Skip) are meant to be inert for both shipped \
         consumers; this says they are not. Do NOT re-bless the golden."
    );
}

#[test]
fn balance_fragment_output_is_byte_identical_to_the_golden() {
    let dir = goldens_dir().join("balanced");
    let entries = corpus();
    assert!(!entries.is_empty(), "the corpus is empty");
    for (name, src) in entries {
        let path = dir.join(format!("{name}.txt"));
        let expected = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("{} should be readable: {e}", path.display()));
        assert_eq!(
            balance_fragment(&src),
            expected,
            "balance_fragment moved for `{name}` — the render path's output \
             is not what it was before the token change"
        );
    }
}

/// Regenerate both goldens. `#[ignore]`d **and** env-var interlocked so no
/// ordinary run — including `--include-ignored` — can rewrite a pin. Run it
/// deliberately, and only when the corpus itself changes — never to turn a
/// red pin green:
///
/// `TRANSYNC_REGEN_GOLDENS=1 cargo test -p transync-html regenerate_goldens -- --ignored --test-threads=4`
#[test]
#[ignore = "writes the goldens; run explicitly with --ignored"]
fn regenerate_goldens() {
    if std::env::var("TRANSYNC_REGEN_GOLDENS").as_deref() != Ok("1") {
        panic!("set TRANSYNC_REGEN_GOLDENS=1 to confirm intentional regeneration");
    }
    let dir = goldens_dir();
    std::fs::create_dir_all(dir.join("balanced")).expect("goldens/balanced should be creatable");
    std::fs::write(dir.join("token-stream.txt"), render_token_golden())
        .expect("token-stream.txt should be writable");
    for (name, src) in corpus() {
        std::fs::write(
            dir.join("balanced").join(format!("{name}.txt")),
            balance_fragment(&src),
        )
        .expect("a balanced golden should be writable");
    }
}
