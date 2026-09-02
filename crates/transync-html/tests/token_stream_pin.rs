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
//! moved.** The one sanctioned exception is a deliberate, reviewed tokenizer
//! **or walk** change landed through the two-bless protocol the `stray-*`
//! and `selfclose-*` entries document (ti 549b20 task 8; ti 490d97 wave 1):
//! bless the broken behaviour first, fix, re-bless, and review the diff
//! between the blessings as the record of the movement. The exception names
//! the walk as well as the tokenizer because `balance_fragment`'s output can
//! move with `scan_tags` untouched — `walk_elements` decides what counts as
//! open, and the balanced goldens record its decisions. The prohibition on
//! blessing a red pin *instead of* reviewing it stands everywhere else.
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
/// doctype spellings, plus (task 8) the three stray-markup constructions
/// of ti 549b20. The twelve wave-0-era entries tokenize the same way
/// before and after wave 0's token change, which is what made this golden
/// a valid before/after comparison for that change.
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
///
/// The three `stray-*` entries are different in kind (ti 549b20, task 8):
/// they are the corpus learning a blind spot. Until task 8 no entry held a
/// bare quote in attribute position, a stray `=` in attribute-name
/// position, or an unterminated tag, so the pin could not see the
/// scanner's stray-markup divergences from a real browser. Their goldens
/// are blessed twice, deliberately: the commit that adds them blesses the
/// BROKEN scanner's behaviour — empty stream, input passed through the
/// balancer unchanged — and the fix re-blesses them through the same
/// interlocked hatch, so the diff between the two blessings is the
/// reviewable record of exactly how tokenization changed. Unlike every
/// entry above, they exist because their tokenization moved inside wave 0.
///
/// The five `selfclose-*` entries are the same move one layer up (ti
/// 490d97 wave 1). Until now no entry held a self-closing spelling of any
/// tag — the golden contains zero `:true` tokens — so the pin could not see
/// that the WALK read the `/` the way XML means it: it refused to push a
/// flagged non-void tag, which made the author's own closing tag an orphan
/// and let the balancer delete it. They are blessed twice for the reason
/// the `stray-*` entries are, but the diff between the blessings records a
/// **walk** movement rather than a tokenizer one — `token-stream.txt` does
/// not move at all here, because `scan_tags` is not touched.
/// `selfclose-svg` and `selfclose-svg-root` are no-regression guards rather
/// than blind spots: foreign content and the `<svg>`/`<math>` start tags
/// are the two places HTML really does honour the flag, so they must pass
/// through the balancer unchanged on both sides of the fix.
///
/// The `tagname-*` and `endtag-*` entries are the third blind spot, one
/// tokenizer state EARLIER than the attribute machine `stray-*` taught (ti
/// `e20490`). Until now no entry held a tag name containing a quote or an
/// `=`, and none held `</` before a non-letter, so the pin could not see two
/// divergences from HTML's tag-open / end-tag-open / tag-name states:
///
/// * **`tagname-plant`** is the one with a security shape. HTML's tag-name
///   state consumes every byte up to whitespace, `/` or `>` INTO the element
///   name, so a browser reads `<divq"x=" data-sync-id="v">` as an element
///   named `divq"x="` carrying a REAL `data-sync-id`. The scanner stopped the
///   name at `divq` and attribute-walked the rest, which buried the plant
///   inside a phantom quoted value that `collect_reserved_attr_spans` never
///   examines as a name — so the strip left a live anchor standing. It is a
///   remainder rather than a re-opened bypass only because a divergent tag
///   name necessarily contains a byte no allowlisted element name has, and
///   the pane path's DOMPurify mount drops the whole element; a consumer that
///   does not sanitize does not inherit that.
/// * **`endtag-*`** is the phantom-structure class instead. `</` before a
///   non-letter is a parse error opening a bogus comment to the first `>`, so
///   a browser mints zero elements; the scanner emitted an `Open` and the
///   balancer owed it a closer the browser reads as orphan junk.
///
/// The `foreign-*` entries are the fourth blind spot, and the first one that
/// is about WHERE a tokenizer state may be entered rather than how it ends (ti
/// `2e2453`). Until now every raw-text entry in the corpus — `rcdata-title`,
/// `rcdata-textarea`, `script-raw-text` — sat in HTML content, where entering
/// that state unconditionally is right. None sat inside `<svg>` or `<math>`,
/// where a browser does not enter it at all: in foreign content `script`,
/// `style`, `textarea` and `title` are ordinary foreign elements, so their
/// contents ARE markup and their self-closing `/` IS honoured. Both
/// divergences were measured in real Chromium on ti `490d97` wave 1:
///
/// * **`foreign-title-markup`** — `<svg><title>a<b>c</b></title>` mints a real
///   `<b>`. The scanner read the title's interior as RCDATA, so the `<b>` and
///   its closer never reached the walk at all, and SVG's `title` could not be
///   listed as the HTML integration point the spec says it is.
/// * **`foreign-script-selfclose`** — `<svg><script/>x` leaves the script
///   EMPTY and `x` a sibling. The scanner entered raw text and swallowed `x`
///   to EOF, so the balancer owed the fragment a `</script>` that closes
///   nothing a browser opened.
///
/// **`foreign-integration-rawtext` is the opposite guard.** Inside an HTML
/// integration point the children are HTML content again, so `<script>` there
/// *does* hold raw text. It must pass through both goldens unchanged on both
/// sides of the fix — the same no-regression role `selfclose-svg` plays for
/// the self-closing flag. Suppressing raw text on a plain `foreign_depth > 0`
/// would move it, which is precisely why the scanner grew a mode STACK rather
/// than a counter.
///
/// The `foreignvoid-*` entries are the fifth, and they move the BALANCER
/// rather than the token stream (ti `48f3c6`). `is_void` was applied
/// globally, foreign content included, so a name HTML makes void was never
/// pushed no matter where it appeared. In foreign content there are no void
/// elements — "any other start tag" inserts a foreign element and only the
/// self-closing flag pops it — so `<svg><link>` is a genuine closable SVG
/// element:
///
/// * **`foreignvoid-closer`** — `<svg><link>a</link>b</svg>`. The author's
///   `</link>` was classified an orphan and DELETED from the pane, silently
///   editing what the reader sees.
/// * **`foreignvoid-unclosed`** — `<svg><link>a`. The other half: no closer
///   was owed, so the fragment closed the `svg` around an element that stayed
///   open.
///
/// **`foreignvoid-br-guard` is the entry that made the fix safe to make at
/// all.** The trade recorded on `48f3c6` was that pushing void names inside
/// foreign content risks appending `</br>`, which HTML's end-tag-`br` rule
/// turns back into a fresh `<br>` — the balancer minting structure instead of
/// repairing it. It cannot happen: `br` is in `FOREIGN_BREAKOUT_TAGS`, so
/// `<svg><br>` tears out of foreign content BEFORE the tag is processed and
/// lands in HTML content, where `is_void` still wins. Same for the other four
/// void names that are also breakout tags (`embed`, `hr`, `img`, `meta`).
/// This entry must not move on either side of the fix, and it is the evidence
/// that DCR-0041's breakout model is what retired the hazard.
///
/// The two `image-*` entries were added by ti `4882ac` to pin an existing
/// decision that nothing pinned, and their doc said they "must stay
/// byte-identical unless someone teaches `scan_tags` HTML's tag-name
/// substitutions, which is the whole point of writing them down". **Someone
/// did, the next day** (ti `e923ef`), so they were blessed once against the
/// unrenamed scanner and once after — which is the two-bless protocol arriving
/// by the route the entries were written to anticipate rather than by a
/// planned wave.
///
/// The movement between those blessings is the record: `image-html` went
/// `image` → `img` in both the inventory and the stream, and its balanced
/// output lost the invented `</image>` it was owed while the element still
/// opened. **`image-foreign` did not move**, and that is the load-bearing
/// half: the substitution is an HTML-CONTENT rule, so SVG's real `<image>`
/// keeps the author's name and their closer. Same line this crate has now
/// drawn four times — raw text (ti `2e2453`), voidness (ti `48f3c6`), the
/// name boundary (ti `e20490`), and now the rename.
///
/// Blessed twice for the reason `stray-*` is: the commit that adds them
/// records the BROKEN tokenization, the fix re-blesses through the same
/// interlock, and the diff between the two blessings is the reviewable record.
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
    ("stray-quote-bare", "<div \"> <p data-sync-id=\"v\">x</p>"),
    (
        "stray-quote-doubled",
        "<div a=\"x\"\" data-sync-id=\"v\">y</div>",
    ),
    ("stray-equals", "<div =\"> data-sync-id=\"v\">x</div>"),
    ("selfclose-div", "<div class=\"a\">\n<div/>y</div>\n</div>"),
    ("selfclose-span", "<span/>tail"),
    ("selfclose-svg", "<svg><rect/><circle/></svg>"),
    ("selfclose-svg-root", "<svg/>after"),
    ("selfclose-p", "<p/>a"),
    // ti `e20490` divergence 1 — HTML's tag-name state.
    ("tagname-plant", "<divq\"x=\" data-sync-id=\"v\">z"),
    ("tagname-equals", "<div=x data-sync-id=\"v\">y"),
    ("tagname-quote-only", "<div\"x>y</div\"x>"),
    // ti `e20490` divergence 2 — HTML's end-tag-open state.
    ("endtag-digit", "</1 <div>x"),
    ("endtag-space", "</ <div>x"),
    ("endtag-empty", "</>x"),
    // ti `2e2453` — raw text is an HTML-content state, not a global one.
    ("foreign-title-markup", "<svg><title>a<b>c</b></title>"),
    ("foreign-script-selfclose", "<svg><script/>x"),
    (
        "foreign-integration-rawtext",
        "<svg><foreignObject><script>a<b>c</script></foreignObject></svg>",
    ),
    // ti `48f3c6` — voidness is an HTML-content rule too.
    ("foreignvoid-closer", "<svg><link>a</link>b</svg>"),
    ("foreignvoid-unclosed", "<svg><link>a"),
    ("foreignvoid-br-guard", "<svg><br>x"),
    // ti `4882ac` — a documented exclusion, made falsifiable.
    ("image-html", "<image>x"),
    ("image-foreign", "<svg><image>a</image>b</svg>"),
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
         consumers; this says they are not. Do NOT re-bless the golden — \
         unless this is a deliberate, reviewed tokenizer or walk change \
         landing through the two-bless protocol (ti 549b20 task 8; ti \
         490d97 wave 1), in which case this red IS the deliverable and the \
         diff between the blessings is its record. Absent that, find out \
         which region moved."
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
             is not what it was before the token change. Do NOT re-bless the \
             golden — unless this is a deliberate, reviewed tokenizer or walk \
             change landing through the two-bless protocol (ti 549b20 task 8; \
             ti 490d97 wave 1), in which case this red IS the deliverable. \
             This is the message a WALK change reaches first: `walk_elements` \
             can move this output with `scan_tags` byte-identical, so a green \
             token-stream pin is not evidence that nothing moved."
        );
    }
}

/// Regenerate both goldens. `#[ignore]`d **and** env-var interlocked so no
/// ordinary run — including `--include-ignored` — can rewrite a pin. Run it
/// deliberately, and only when the corpus itself changes or a deliberate,
/// reviewed tokenizer **or walk** change lands through the two-bless
/// protocol (ti 549b20 task 8; ti 490d97 wave 1) — never to turn a red pin
/// green as a shortcut past reviewing what moved:
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
