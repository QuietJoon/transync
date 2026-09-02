//! The weld that keeps the backend contained.
//!
//! `backend.rs` says `whichlang` must be named in exactly one file. Prose does
//! not enforce itself, so this reads the crate's own sources and checks.

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    fn src_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
    }

    /// Sources with comment lines removed, because the rule is about CODE.
    ///
    /// The first draft of this weld scanned raw bytes and failed on its own
    /// crate: `lib.rs` names the backend in prose — it has to, to explain why it
    /// is contained — and this file names it in an assertion message. A weld
    /// that fires on its own documentation trains people to silence it, so it
    /// reads what the compiler reads instead.
    ///
    /// Line-based and therefore approximate: it would miss a reference inside a
    /// block comment, and this crate has none. It is not approximate in the
    /// direction that matters — no real `use` or path expression survives it.
    fn code_only(body: &str) -> String {
        body.lines()
            .filter(|l| !l.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Every module except this one. The checker is not a consumer of the
    /// backend, and exempting it by name is honest where exempting it by a
    /// cleverly-spelled needle would not be.
    fn rust_sources() -> Vec<(String, String)> {
        let mut out = Vec::new();
        for entry in fs::read_dir(src_dir()).expect("src/ should be readable") {
            let path = entry.expect("a readable dir entry").path();
            if path.extension().is_some_and(|e| e == "rs") {
                let name = path
                    .file_name()
                    .expect("a file has a name")
                    .to_string_lossy()
                    .into_owned();
                if name == "containment.rs" {
                    continue;
                }
                out.push((
                    name,
                    fs::read_to_string(&path).expect("a readable source file"),
                ));
            }
        }
        assert!(
            out.len() >= 3,
            "expected the crate's three shipped modules, found {} — the scan broke, \
             it did not get simpler",
            out.len()
        );
        out
    }

    /// The containment itself: one file may name the backend.
    ///
    /// If this fails, the question is not "how do I silence it" but "does the
    /// new call site put `whichlang`'s types on our public API" — because that
    /// is what the single-file rule exists to prevent. `whichlang` is 0.1.x, so
    /// its every release may break; a leaked type would make its semver ours.
    #[test]
    fn backend_is_the_only_module_that_names_whichlang() {
        let offenders: Vec<String> = rust_sources()
            .into_iter()
            .filter(|(name, body)| name != "backend.rs" && code_only(body).contains("whichlang"))
            .map(|(name, _)| name)
            .collect();
        assert!(
            offenders.is_empty(),
            "only backend.rs may name whichlang; found it in {offenders:?}. \
             Route the call through backend::detect rather than widening the surface."
        );
    }

    /// The other half: no public signature may mention it either. A `pub fn`
    /// returning a backend type would leak through `backend.rs` itself, which
    /// the file-count check above cannot see.
    #[test]
    fn no_public_item_names_the_backend_in_its_signature() {
        for (name, body) in rust_sources() {
            for line in code_only(&body).lines() {
                let line = line.trim_start();
                if line.starts_with("pub fn")
                    || line.starts_with("pub struct")
                    || line.starts_with("pub enum")
                {
                    assert!(
                        !line.contains("whichlang"),
                        "{name}: public item names the backend: {line}"
                    );
                }
            }
        }
    }
}
