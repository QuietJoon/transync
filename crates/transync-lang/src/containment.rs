//! The weld that keeps the backend contained.
//!
//! `backend.rs` says `whichlang` is named in exactly one file, and there only
//! inside function bodies. Prose does not enforce itself, so this parses the
//! crate's own sources and checks (R0012-0021).
//!
//! **Parsed, not scraped.** Every `*.rs` file under `src/`, at any depth, goes
//! through `syn::parse_file`, and the rule is asked of identifiers. Comments
//! are gone after parsing and string literals are not identifiers, so prose
//! that names the backend — `lib.rs` has to, to explain the containment —
//! never trips it, and no file needs an exemption. A file that does not parse
//! is a violation rather than a skip.
//!
//! The rule, and why it is stricter than "no public item":
//!
//! - outside `backend.rs`, any identifier `whichlang` is a violation — a `use`,
//!   a path, an `extern crate`, a macro argument;
//! - inside `backend.rs`, only an occurrence inside a function **body** is
//!   allowed. A signature, a type alias, a `use` (renaming or not, private or
//!   not), an impl header or member signature, a field, a const, a
//!   `macro_rules!` body — every one of those is a violation, and an item
//!   nested inside a body counts as outside one, because an `impl` written
//!   inside a function is still global. Private items are included on purpose:
//!   telling a private `use` from a re-exportable one is the judgement this
//!   rule exists not to need.
//!
//! What it refuses rather than reads: `#[path]` on a module (directly or
//! through `cfg_attr`) and `include!`, both of which hand the compiler a file
//! this walk never parses.
//!
//! Its one blind spot, named rather than hidden: a macro **invoked inside a
//! body** is opaque tokens to a parser, so a macro there that expanded into a
//! global impl naming the backend would pass. Expansion is the compiler's
//! knowledge, not a parser's.
//!
//! Backstop: each file is also tokenized flat, and every `whichlang` token in
//! it must have been reached by the syntax walk. A construct the walk skips —
//! a syn node whose tokens it does not visit — fails the file instead of
//! hiding an occurrence.

use std::fs;
use std::path::Path;

use proc_macro2::{TokenStream, TokenTree};
use syn::ext::IdentExt;
use syn::visit::{self, Visit};

/// The backend's crate name, as an identifier.
const BACKEND: &str = "whichlang";

/// The one file allowed to name it.
const BACKEND_FILE: &str = "backend.rs";

/// Every `*.rs` file under `root`, at any depth, as `(path, source)` pairs.
/// The path is relative to `root` and `/`-separated; the list is sorted.
pub(crate) fn collect(root: &Path) -> Vec<(String, String)> {
    fn walk(dir: &Path, root: &Path, out: &mut Vec<(String, String)>) {
        let entries = fs::read_dir(dir)
            .unwrap_or_else(|e| panic!("{} should be readable: {e}", dir.display()));
        for entry in entries {
            let path = entry.expect("a readable dir entry").path();
            if path.is_dir() {
                walk(&path, root, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                let rel = path
                    .strip_prefix(root)
                    .expect("the walk stays under its root")
                    .components()
                    .map(|c| c.as_os_str().to_string_lossy().into_owned())
                    .collect::<Vec<_>>()
                    .join("/");
                let body = fs::read_to_string(&path)
                    .unwrap_or_else(|e| panic!("{} should be readable: {e}", path.display()));
                out.push((rel, body));
            }
        }
    }
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out.sort();
    out
}

/// What kind of breach a [`Violation`] is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    /// A file other than `backend.rs` names the backend.
    OutsideBackend,
    /// `backend.rs` names it outside a function body.
    OutsideBody,
    /// A `#[path]` module or an `include!`: code this walk cannot follow.
    Redirect,
    /// The file does not parse, or the walk missed tokens the flat scan saw.
    Unreadable,
}

/// One breach, located by file and enclosing item.
#[derive(Debug, Clone)]
pub(crate) struct Violation {
    pub(crate) kind: Kind,
    pub(crate) message: String,
}

/// One identifier `whichlang` the walk reached.
#[derive(Debug, Clone)]
pub(crate) struct Hit {
    /// Inside a function body (and not inside an item nested in one).
    pub(crate) in_body: bool,
    /// The enclosing item chain, outermost first, e.g. `fn detect`.
    pub(crate) scope: String,
}

/// The syntax walk over one parsed file.
#[derive(Default)]
struct Walk {
    in_body: bool,
    scope: Vec<String>,
    hits: Vec<Hit>,
    redirects: Vec<String>,
}

impl Walk {
    fn scope(&self) -> String {
        if self.scope.is_empty() {
            "file root".to_string()
        } else {
            self.scope.join(" > ")
        }
    }

    fn hit(&mut self) {
        let scope = self.scope();
        self.hits.push(Hit {
            in_body: self.in_body,
            scope,
        });
    }

    fn in_scope(&mut self, label: String, f: impl FnOnce(&mut Self)) {
        self.scope.push(label);
        f(self);
        self.scope.pop();
    }
}

fn label(item: &syn::Item) -> String {
    match item {
        syn::Item::Const(i) => format!("const {}", i.ident),
        syn::Item::Enum(i) => format!("enum {}", i.ident),
        syn::Item::ExternCrate(i) => format!("extern crate {}", i.ident),
        syn::Item::Fn(i) => format!("fn {}", i.sig.ident),
        syn::Item::ForeignMod(_) => "extern block".to_string(),
        syn::Item::Impl(_) => "impl block".to_string(),
        syn::Item::Macro(i) => match &i.ident {
            Some(name) => format!("macro_rules! {name}"),
            None => "macro invocation".to_string(),
        },
        syn::Item::Mod(i) => format!("mod {}", i.ident),
        syn::Item::Static(i) => format!("static {}", i.ident),
        syn::Item::Struct(i) => format!("struct {}", i.ident),
        syn::Item::Trait(i) => format!("trait {}", i.ident),
        syn::Item::TraitAlias(i) => format!("trait alias {}", i.ident),
        syn::Item::Type(i) => format!("type {}", i.ident),
        syn::Item::Union(i) => format!("union {}", i.ident),
        syn::Item::Use(_) => "use declaration".to_string(),
        _ => "unparsed item".to_string(),
    }
}

/// Every identifier `whichlang` in `tokens`, at any group depth.
fn count_in_tokens(tokens: &TokenStream) -> usize {
    tokens
        .clone()
        .into_iter()
        .map(|tt| match tt {
            TokenTree::Ident(id) => usize::from(id.unraw() == BACKEND),
            TokenTree::Group(g) => count_in_tokens(&g.stream()),
            TokenTree::Punct(_) | TokenTree::Literal(_) => 0,
        })
        .sum()
}

/// `path` or `cfg_attr(…, path = …)`: an attribute that moves a module's file.
fn redirects_module(attr: &syn::Attribute) -> bool {
    let path = attr.path();
    if path.is_ident("path") {
        return true;
    }
    if path.is_ident("cfg_attr")
        && let syn::Meta::List(list) = &attr.meta
    {
        return list
            .tokens
            .clone()
            .into_iter()
            .any(|tt| matches!(tt, TokenTree::Ident(id) if id == "path"));
    }
    false
}

impl<'ast> Visit<'ast> for Walk {
    fn visit_item(&mut self, item: &'ast syn::Item) {
        // An item is never inside a body, even when it is written in one: an
        // `impl` inside a function is still global.
        let outer = std::mem::replace(&mut self.in_body, false);
        self.in_scope(label(item), |w| visit::visit_item(w, item));
        self.in_body = outer;
    }

    fn visit_item_fn(&mut self, f: &'ast syn::ItemFn) {
        for attr in &f.attrs {
            self.visit_attribute(attr);
        }
        self.visit_visibility(&f.vis);
        self.visit_signature(&f.sig);
        let outer = std::mem::replace(&mut self.in_body, true);
        self.visit_block(&f.block);
        self.in_body = outer;
    }

    fn visit_impl_item_fn(&mut self, f: &'ast syn::ImplItemFn) {
        self.in_scope(format!("fn {}", f.sig.ident), |w| {
            for attr in &f.attrs {
                w.visit_attribute(attr);
            }
            w.visit_visibility(&f.vis);
            w.visit_signature(&f.sig);
            let outer = std::mem::replace(&mut w.in_body, true);
            w.visit_block(&f.block);
            w.in_body = outer;
        });
    }

    fn visit_trait_item_fn(&mut self, f: &'ast syn::TraitItemFn) {
        self.in_scope(format!("fn {}", f.sig.ident), |w| {
            for attr in &f.attrs {
                w.visit_attribute(attr);
            }
            w.visit_signature(&f.sig);
            if let Some(block) = &f.default {
                let outer = std::mem::replace(&mut w.in_body, true);
                w.visit_block(block);
                w.in_body = outer;
            }
        });
    }

    fn visit_item_mod(&mut self, m: &'ast syn::ItemMod) {
        if m.attrs.iter().any(redirects_module) {
            self.redirects
                .push(format!("`mod {}` is redirected by #[path]", m.ident));
        }
        visit::visit_item_mod(self, m);
    }

    fn visit_macro(&mut self, mac: &'ast syn::Macro) {
        if mac
            .path
            .segments
            .last()
            .is_some_and(|s| s.ident == "include")
        {
            let scope = self.scope();
            self.redirects.push(format!(
                "`{scope}` uses include!, whose file this walk never parses"
            ));
        }
        visit::visit_macro(self, mac);
    }

    fn visit_ident(&mut self, ident: &'ast proc_macro2::Ident) {
        if ident.unraw() == BACKEND {
            self.hit();
        }
    }

    // syn hands macro arguments, attribute lists and verbatim nodes over as
    // unparsed tokens, through this one method; its default ignores them.
    fn visit_token_stream(&mut self, tokens: &'ast TokenStream) {
        for _ in 0..count_in_tokens(tokens) {
            self.hit();
        }
    }
}

/// Walk one file, or say why it cannot be walked.
pub(crate) fn walk(src: &str) -> Result<(Vec<Hit>, Vec<String>), String> {
    let file = syn::parse_file(src).map_err(|e| format!("does not parse: {e}"))?;
    let mut walk = Walk::default();
    walk.visit_file(&file);
    let flat: TokenStream = src.parse().map_err(|e| format!("does not tokenize: {e}"))?;
    let seen = count_in_tokens(&flat);
    if seen > walk.hits.len() {
        return Err(format!(
            "{} occurrence(s) of `{BACKEND}` the syntax walk did not reach",
            seen - walk.hits.len()
        ));
    }
    Ok((walk.hits, walk.redirects))
}

/// Every breach of the containment rule across `files` (`(path, source)`
/// pairs, paths as [`collect`] returns them).
pub(crate) fn violations(files: &[(String, String)]) -> Vec<Violation> {
    let mut out = Vec::new();
    for (path, src) in files {
        let (hits, redirects) = match walk(src) {
            Ok(found) => found,
            Err(why) => {
                out.push(Violation {
                    kind: Kind::Unreadable,
                    message: format!("{path}: {why}"),
                });
                continue;
            }
        };
        for why in redirects {
            out.push(Violation {
                kind: Kind::Redirect,
                message: format!("{path}: {why}"),
            });
        }
        let is_backend = path == BACKEND_FILE;
        for hit in hits {
            if !is_backend {
                out.push(Violation {
                    kind: Kind::OutsideBackend,
                    message: format!(
                        "{path}: `{}` names {BACKEND}; only {BACKEND_FILE} may",
                        hit.scope
                    ),
                });
            } else if !hit.in_body {
                out.push(Violation {
                    kind: Kind::OutsideBody,
                    message: format!(
                        "{path}: `{}` names {BACKEND} outside a function body",
                        hit.scope
                    ),
                });
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn src_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
    }

    fn fixture_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/containment")
    }

    fn files(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(p, s)| (p.to_string(), s.to_string()))
            .collect()
    }

    fn of_kind(found: &[Violation], kind: Kind) -> Vec<&str> {
        found
            .iter()
            .filter(|v| v.kind == kind)
            .map(|v| v.message.as_str())
            .collect()
    }

    /// The containment itself: one file may name the backend.
    ///
    /// If this fails, the question is not "how do I silence it" but "does the
    /// new call site put `whichlang`'s types on our public API" — because that
    /// is what the single-file rule exists to prevent. `whichlang` is 0.1.x, so
    /// its every release may break; a leaked type would make its semver ours.
    #[test]
    fn backend_is_the_only_module_that_names_whichlang() {
        let found = violations(&collect(&src_dir()));
        let outside = of_kind(&found, Kind::OutsideBackend);
        assert!(
            outside.is_empty(),
            "only backend.rs may name whichlang: {outside:#?}. Route the call through \
             backend::detect rather than widening the surface."
        );
        let other: Vec<&str> = found
            .iter()
            .filter(|v| v.kind != Kind::OutsideBackend && v.kind != Kind::OutsideBody)
            .map(|v| v.message.as_str())
            .collect();
        assert!(other.is_empty(), "the walk could not vouch for: {other:#?}");
    }

    /// The other half: inside `backend.rs`, only function bodies. A
    /// signature, alias, `use`, impl or field naming the backend could carry
    /// its type onto the public API through `backend.rs` itself.
    #[test]
    fn backend_names_whichlang_only_inside_function_bodies() {
        let found = violations(&collect(&src_dir()));
        let outside_body = of_kind(&found, Kind::OutsideBody);
        assert!(outside_body.is_empty(), "{outside_body:#?}");
    }

    /// Anti-vacuity: the walk read the shipped tree and found the backend
    /// where it is allowed. A parse that silently saw nothing, or a manifest
    /// that renamed the dependency, would leave every rule above vacuously
    /// green.
    #[test]
    fn the_walk_sees_the_shipped_tree() {
        let collected = collect(&src_dir());
        let names: Vec<&str> = collected.iter().map(|(n, _)| n.as_str()).collect();
        for expected in ["backend.rs", "containment.rs", "lib.rs", "script.rs"] {
            assert!(
                names.contains(&expected),
                "the walk must read {expected}, got {names:?}"
            );
        }
        let (_, backend) = collected
            .iter()
            .find(|(n, _)| n == BACKEND_FILE)
            .expect("backend.rs is collected");
        let (hits, _) = walk(backend).expect("backend.rs parses");
        assert!(
            hits.iter().any(|h| h.in_body && h.scope == "fn detect"),
            "backend.rs must name whichlang inside detect's body, got {hits:?}"
        );
    }

    // ---- R0012-0021 regression fixtures -----------------------------------

    /// (a) A public alias in backend.rs, re-exported at the crate root.
    #[test]
    fn a_public_alias_reexported_from_the_root_is_caught() {
        let found = violations(&files(&[
            ("backend.rs", "pub type External = whichlang::Lang;\n"),
            ("lib.rs", "mod backend;\npub use backend::External;\n"),
        ]));
        assert_eq!(
            of_kind(&found, Kind::OutsideBody),
            ["backend.rs: `type External` names whichlang outside a function body"],
            "the alias leaked whichlang::Lang"
        );
    }

    /// (b) A signature split across lines.
    #[test]
    fn a_multiline_signature_is_caught() {
        let found = violations(&files(&[(
            "backend.rs",
            "pub fn raw(\n    text: &str,\n) -> whichlang::Lang {\n    whichlang::detect_language(text)\n}\n",
        )]));
        assert_eq!(
            of_kind(&found, Kind::OutsideBody),
            ["backend.rs: `fn raw` names whichlang outside a function body"],
            "the return type leaked whichlang::Lang; the body's call is allowed"
        );
    }

    /// (c) A trait impl puts the backend type on a public type's API with no
    /// `pub` keyword anywhere.
    #[test]
    fn a_trait_impl_naming_the_backend_is_caught() {
        let found = violations(&files(&[(
            "backend.rs",
            "impl From<whichlang::Lang> for crate::Language {\n    fn from(_: whichlang::Lang) -> Self {\n        todo!()\n    }\n}\n",
        )]));
        assert_eq!(
            of_kind(&found, Kind::OutsideBody),
            [
                "backend.rs: `impl block` names whichlang outside a function body",
                "backend.rs: `impl block > fn from` names whichlang outside a function body",
            ],
        );
    }

    /// (c') The same impl written inside a function body is still global.
    #[test]
    fn an_impl_inside_a_body_is_still_outside_one() {
        let found = violations(&files(&[(
            "backend.rs",
            "fn f() {\n    impl From<whichlang::Lang> for crate::Language {\n        fn from(_: whichlang::Lang) -> Self { todo!() }\n    }\n}\n",
        )]));
        assert_eq!(of_kind(&found, Kind::OutsideBody).len(), 2, "{found:#?}");
    }

    /// (d) A renaming `use` in backend.rs lets every other file reach the
    /// backend without spelling its name. Caught at the alias, where the name
    /// still appears.
    #[test]
    fn a_renaming_use_in_backend_is_caught() {
        let found = violations(&files(&[
            ("backend.rs", "pub(crate) use whichlang as wl;\n"),
            (
                "script.rs",
                "fn f(t: &str) -> crate::backend::wl::Lang { crate::backend::wl::detect_language(t) }\n",
            ),
        ]));
        assert_eq!(
            of_kind(&found, Kind::OutsideBody),
            ["backend.rs: `use declaration` names whichlang outside a function body"],
        );
    }

    /// (d') `extern crate … as` is the same alias by an older spelling, and a
    /// raw identifier is the same name.
    #[test]
    fn extern_crate_and_raw_identifiers_are_caught() {
        let found = violations(&files(&[
            ("backend.rs", "extern crate whichlang as wl;\n"),
            ("lib.rs", "use r#whichlang::Lang;\n"),
        ]));
        assert_eq!(of_kind(&found, Kind::OutsideBody).len(), 1, "{found:#?}");
        assert_eq!(of_kind(&found, Kind::OutsideBackend).len(), 1, "{found:#?}");
    }

    /// (e) A child module one directory down, file-as-module style.
    #[test]
    fn a_nested_module_is_read_and_caught() {
        let found = violations(&files(&[("script/detail.rs", "use whichlang::Lang;\n")]));
        assert_eq!(
            of_kind(&found, Kind::OutsideBackend),
            ["script/detail.rs: `use declaration` names whichlang; only backend.rs may"],
        );
    }

    /// (e') A `backend/` directory is not `backend.rs`.
    #[test]
    fn only_the_backend_file_itself_is_exempt() {
        let found = violations(&files(&[(
            "backend/inner.rs",
            "fn f() { whichlang::detect_language(\"\"); }\n",
        )]));
        assert_eq!(of_kind(&found, Kind::OutsideBackend).len(), 1, "{found:#?}");
    }

    /// (f) `#[path]` points the compiler at a file the walk never parses.
    #[test]
    fn a_path_redirect_is_refused() {
        for src in [
            "#[path = \"../x.rs\"]\nmod x;\n",
            "#[cfg_attr(test, path = \"../x.rs\")]\nmod x;\n",
        ] {
            let found = violations(&files(&[("lib.rs", src)]));
            assert_eq!(
                of_kind(&found, Kind::Redirect),
                ["lib.rs: `mod x` is redirected by #[path]"],
                "{src:?}"
            );
        }
    }

    /// (f') `include!` does the same from inside an item.
    #[test]
    fn an_include_is_refused() {
        let found = violations(&files(&[("lib.rs", "include!(\"../x.rs\");\n")]));
        assert_eq!(
            of_kind(&found, Kind::Redirect),
            ["lib.rs: `macro invocation` uses include!, whose file this walk never parses"],
        );
    }

    /// Macro arguments and attribute lists are tokens, not syntax; the walk
    /// reads them anyway.
    #[test]
    fn macro_and_attribute_tokens_are_read() {
        let found = violations(&files(&[
            (
                "backend.rs",
                "macro_rules! leak { () => { whichlang::Lang } }\n#[derive(whichlang::Marker)]\nstruct S;\n",
            ),
            ("lib.rs", "fn f() { let _ = stringify!(whichlang); }\n"),
        ]));
        assert_eq!(of_kind(&found, Kind::OutsideBody).len(), 2, "{found:#?}");
        assert_eq!(of_kind(&found, Kind::OutsideBackend).len(), 1, "{found:#?}");
    }

    /// A file the parser rejects is a violation, never a skip.
    #[test]
    fn a_file_that_does_not_parse_is_a_violation() {
        let found = violations(&files(&[("lib.rs", "fn broken( {\n")]));
        assert_eq!(of_kind(&found, Kind::Unreadable).len(), 1, "{found:#?}");
    }

    /// The walker reaches a child module one directory down.
    #[test]
    fn the_walk_reaches_nested_modules() {
        let collected = collect(&fixture_dir().join("nested"));
        let names: Vec<&str> = collected.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, ["top.rs", "top/inner.rs"]);
        assert_eq!(
            of_kind(&violations(&collected), Kind::OutsideBackend),
            ["top/inner.rs: `use declaration` names whichlang; only backend.rs may"],
        );
    }

    /// (g) Negatives: today's backend shape, and prose / literals naming it.
    #[test]
    fn the_shipped_shape_and_prose_mentions_pass() {
        let found = violations(&files(&[
            (
                "backend.rs",
                "pub(crate) fn detect(text: &str) -> Option<Language> {\n    Some(match whichlang::detect_language(text) {\n        whichlang::Lang::Eng => Language::English,\n    })\n}\n",
            ),
            (
                "lib.rs",
                "/// Backed by whichlang.\n/* whichlang */\nconst NAME: &str = \"whichlang\";\n",
            ),
        ]));
        assert!(found.is_empty(), "{found:#?}");
    }
}
