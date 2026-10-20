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

/// Identifier soup of the kind an agent's answer carries around its prose.
/// Three copies push any of the one-sentence samples below into the
/// `Minority` band — the script present but under a fifth of the letters —
/// which is the band the script count hands to the backend.
const IDENTIFIERS: &str = "getUserAccountBalanceFromDatabase parseIncomingRequestHeaders \
                           validateCustomerPaymentInformation";

fn diluted(prose: &str) -> String {
    format!("{prose} {}", [IDENTIFIERS; 3].join(" "))
}

/// Ukrainian (і U+0456, ї U+0457), written in Cyrillic.
const UKRAINIAN: &str = "Це український текст, який потрібно перекласти.";
/// Persian (ی U+06CC, ک U+06A9), written in the Arabic script.
const PERSIAN: &str = "این یک متن فارسی است که باید ترجمه شود.";
/// Marathi carrying ळ U+0933, written in Devanagari.
const MARATHI: &str = "सकाळी शाळेत जाताना मला वेळ मिळाला नाही, म्हणून मी घरीच अभ्यास केला.";
/// Russian, including ё U+0451 — a letter of the Russian alphabet.
const RUSSIAN: &str = "Это русский текст, который уже переведён на нужный язык.";
/// Arabic, with Arabic yeh U+064A and alef maqsura U+0649.
const ARABIC: &str = "هذا نص عربي تمت ترجمته بالفعل إلى اللغة المطلوبة.";
/// Hindi.
const HINDI: &str = "यह हिंदी में लिखा गया पाठ है जिसका अनुवाद पहले ही हो चुका है।";

/// R0012-0014. Cyrillic, the Arabic script and Devanagari are exclusive only
/// *within the backend's set*; Ukrainian, Persian and Marathi write them too.
/// Each sample carries a letter its target's orthography never uses, so the
/// gate must not call it the target — and calling it the target is the one
/// unsafe direction, because `AlreadyTarget` cancels the run.
#[test]
fn another_language_in_the_targets_script_is_not_the_target() {
    let wrong: Vec<String> = [
        (Language::Russian, UKRAINIAN),
        (Language::Arabic, PERSIAN),
        (Language::Hindi, MARATHI),
    ]
    .into_iter()
    .map(|(target, text)| (target, text, Gate::new(target).verdict(text)))
    .filter(|(_, _, v)| *v != Verdict::Translate(Reason::ForeignOrthography))
    .map(|(target, text, v)| format!("{target:?} gate, {text:?}: {v:?}"))
    .collect();
    assert!(
        wrong.is_empty(),
        "each sample carries a letter its target never uses, so none is the target: {wrong:#?}"
    );
}

/// The same samples diluted into the `Minority` band. The backend sees only
/// 128-codepoint block buckets outside ASCII, so it names Russian, Arabic
/// and Hindi for them; the letter screen has to run before that hand-off,
/// not only before the `Dominant` shortcut.
#[test]
fn a_foreign_letter_is_decisive_before_the_backend_is_asked() {
    let wrong: Vec<String> = [
        (Language::Russian, UKRAINIAN),
        (Language::Arabic, PERSIAN),
        (Language::Hindi, MARATHI),
    ]
    .into_iter()
    .map(|(target, prose)| (target, prose, Gate::new(target).verdict(&diluted(prose))))
    .filter(|(_, _, v)| *v != Verdict::Translate(Reason::ForeignOrthography))
    .map(|(target, prose, v)| format!("{target:?} gate, {prose:?} among identifiers: {v:?}"))
    .collect();
    assert!(
        wrong.is_empty(),
        "a foreign letter must settle the verdict before the backend is asked: {wrong:#?}"
    );
}

/// The controls: the screens must not cost the target language itself, on
/// either path. Pure text is still settled by the script count; diluted text
/// still reaches the backend and is still named the target.
#[test]
fn the_shared_script_targets_still_recognize_their_own_language() {
    for (target, text) in [
        (Language::Russian, RUSSIAN),
        (Language::Arabic, ARABIC),
        (Language::Hindi, HINDI),
    ] {
        assert_eq!(
            Gate::new(target).verdict(text),
            Verdict::AlreadyTarget(Basis::ExclusiveScript),
            "{text:?} is {target:?}"
        );
        assert_eq!(
            Gate::new(target).verdict(&diluted(text)),
            Verdict::AlreadyTarget(Basis::Backend),
            "{target:?} prose among identifiers is still {target:?}, and the backend says so"
        );
    }
}

/// The residual misses the input-domain statement names (DCR-0063), pinned so
/// the statement stays true: a language writing the target's script without
/// a letter the target never uses still reads as the target. If this starts
/// failing because a screen grew, the crate doc, `Gate::verdict` and
/// ADR-0028 change with it.
#[test]
fn the_documented_residual_misses_are_still_misses() {
    // Bulgarian writes a subset of the Russian alphabet.
    let bulgarian = "Това е български текст, който трябва да бъде преведен.";
    // Nepali writes no letter Hindi lacks.
    let nepali = "यो नेपाली भाषामा लेखिएको पाठ हो।";
    // Marathi carrying no screened letter: the documented "most Marathi".
    let marathi = "मी घरी जातो.";
    for (target, text) in [
        (Language::Russian, bulgarian),
        (Language::Hindi, nepali),
        (Language::Hindi, marathi),
    ] {
        assert_eq!(
            Gate::new(target).verdict(text),
            Verdict::AlreadyTarget(Basis::ExclusiveScript),
            "{text:?} is outside the gate's input domain and documented as read as {target:?}"
        );
    }
}
