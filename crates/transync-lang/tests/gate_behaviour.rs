//! The gate's behaviour, pinned against the corpus that chose its backend.
//!
//! `benchmark/lang-detect/corpus/` is read **in place**, never copied. Two
//! places holding one corpus is the defect class this workspace keeps removing,
//! and the token-stream pin in `transync-html` already sets the precedent for a
//! test reading fixtures across a package boundary.
//!
//! What is deliberately NOT read: `benchmark/lang-detect/corpus-real/`. It is
//! gitignored — derived from another repository's fixtures — so a test that
//! depended on it would pass or skip depending on who ran it, and a gate that
//! can silently skip is not a gate.

use std::path::{Path, PathBuf};

use transync_lang::{Basis, Gate, Language, Reason, Verdict};

fn corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../benchmark/lang-detect/corpus")
}

fn read(name: &str) -> String {
    let path = corpus().join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "{} should be readable: {e}. Regenerate with \
             `python3 benchmark/lang-detect/scripts/build-corpus-synthetic.py`",
            path.display()
        )
    })
}

/// The corpus the gate is pinned against must actually be there. A missing
/// corpus would otherwise turn every assertion below into a panic message
/// nobody reads as "the fixture set moved".
#[test]
fn the_benchmark_corpus_is_where_this_test_expects_it() {
    assert!(
        corpus().is_dir(),
        "expected the committed benchmark corpus at {}",
        corpus().display()
    );
    assert!(!read("mix000-200.txt").trim().is_empty());
}

/// The headline: pure Korean is skipped, pure English is translated, at every
/// length the corpus carries.
#[test]
fn the_gate_separates_the_two_ends_of_the_sweep() {
    let gate = Gate::new(Language::Korean);
    for n in ["200", "2000"] {
        assert_eq!(
            gate.verdict(&read(&format!("mix000-{n}.txt"))),
            Verdict::AlreadyTarget(Basis::ExclusiveScript),
            "pure Korean at {n} words is already the target, and the free test says so"
        );
        assert_eq!(
            gate.verdict(&read(&format!("mix100-{n}.txt"))),
            Verdict::Translate(Reason::ExclusiveScriptAbsent),
            "pure English at {n} words carries no Hangul at all"
        );
    }
}

/// The property the threshold exists for: a Korean answer stays Korean through
/// a heavy load of English identifiers. This is the expensive direction — a
/// wrong answer here costs a provider call AND emits ko→ko.
#[test]
fn a_korean_answer_survives_dilution_with_english_identifiers() {
    let gate = Gate::new(Language::Korean);
    for pct in ["010", "020", "030", "040"] {
        for n in ["200", "2000"] {
            let v = gate.verdict(&read(&format!("mix{pct}-{n}.txt")));
            assert!(
                !v.should_translate(),
                "mix{pct}-{n}: {pct}% English is still a Korean answer, got {v:?}"
            );
        }
    }
}

/// The other direction, and the reason `Reason::Undecided` is not a third
/// verdict: everything the gate cannot settle still routes to Translate.
#[test]
fn every_uncertain_input_routes_to_translate() {
    let gate = Gate::new(Language::Korean);
    for text in ["", "   \n\t ", "{}();  42  3.14  ---", "|---|---|"] {
        assert_eq!(
            gate.verdict(text),
            Verdict::Translate(Reason::Undecided),
            "text with no letters is not a language judgement: {text:?}"
        );
    }
}

/// Japanese and Chinese must not read as Korean. The script test abstains on
/// them — no Hangul, and for Chinese no kana either — so this is the band the
/// backend was added for.
#[test]
fn other_cjk_languages_are_not_mistaken_for_korean() {
    let gate = Gate::new(Language::Korean);
    for f in ["ja-200.txt", "zh-200.txt"] {
        let v = gate.verdict(&read(f));
        assert!(v.should_translate(), "{f} is not Korean, got {v:?}");
    }
}

/// A target whose script is shared skips the script stage entirely and is
/// answered by the backend. Without this the gate would count a script that
/// cannot discriminate and hand back a confident wrong answer.
#[test]
fn a_latin_target_is_decided_by_the_backend() {
    let english = Gate::new(Language::English);
    assert_eq!(
        english.verdict(&read("mix100-2000.txt")),
        Verdict::AlreadyTarget(Basis::Backend),
        "English text against an English target is already the target"
    );
    assert!(
        english.verdict(&read("mix000-2000.txt")).should_translate(),
        "Korean text against an English target needs translating"
    );
}

/// Romanized Korean is NOT claimed. Every detector in the benchmark failed it,
/// so the gate must not pretend otherwise — and failing toward Translate is the
/// safe failure.
#[test]
fn romanized_korean_is_not_claimed_as_korean() {
    let gate = Gate::new(Language::Korean);
    let v = gate.verdict(&read("ko-romanized-200.txt"));
    assert!(
        v.should_translate(),
        "no candidate in benchmark/lang-detect handled romanized Korean; the gate \
         must fail toward translating rather than claim it, got {v:?}"
    );
}

/// English that merely quotes a Korean sentence is still English.
#[test]
fn a_korean_quotation_does_not_make_an_english_document_korean() {
    let gate = Gate::new(Language::Korean);
    assert!(
        gate.verdict(&read("en-with-ko-quote-200.txt"))
            .should_translate()
    );
}

/// The public surface holds its shape: the verdict carries no language, so
/// there is nothing in it to copy into `detected_source_language`.
#[test]
fn a_verdict_names_no_language() {
    let rendered = format!(
        "{:?}",
        Gate::new(Language::Korean).verdict("이미 한국어입니다")
    );
    for name in ["Korean", "kor", "English"] {
        assert!(
            !rendered.contains(name),
            "a Verdict must not carry a language name — that is what keeps it from \
             becoming the pipeline's answer. Got: {rendered}"
        );
    }
}

/// Round-trip every language through its own codes.
#[test]
fn iso_codes_round_trip_and_reject_region_subtags() {
    for lang in [
        Language::Korean,
        Language::English,
        Language::Japanese,
        Language::Chinese,
    ] {
        assert_eq!(Language::from_iso639(lang.iso639_3()), Some(lang));
    }
    assert_eq!(Language::from_iso639("ko"), Some(Language::Korean));
    assert_eq!(Language::from_iso639("KOR"), Some(Language::Korean));
    // Strict: a region subtag is not a language code here, and guessing would
    // fail in the direction that skips a translation the caller wanted.
    assert_eq!(Language::from_iso639("ko-KR"), None);
    assert_eq!(Language::from_iso639("klingon"), None);
}
