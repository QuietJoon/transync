//! Which language detector should back the "is this already the target
//! language?" gate — ticket `eb1d89`.
//!
//! The gate's job: a consumer captures an agent's final response and wants to
//! skip translating it when the agent already answered in the target language.
//! Getting that wrong in the expensive direction — calling genuine Korean
//! "not Korean" — costs a provider call AND emits ko→ko output, which is the
//! failure the ticket exists to prevent.
//!
//! ## What this measures, and why it is not what the libraries advertise
//!
//! Every candidate markets itself on SHORT-text accuracy. That is not this
//! workload: the consumer states 20 words at the low end and 10,000+ at the
//! high end, and 20 words is already comfortable for a trigram model. What
//! actually separates them is the **flip point** — the proportion of English
//! words at which each stops calling a Korean answer Korean. An agent
//! explaining code in Korean emits a lot of English identifiers, and that
//! answer is still Korean.
//!
//! ## The control
//!
//! `script` is a Hangul codepoint test with no dependency at all. Any library
//! that cannot beat it here is carrying its weight for nothing. Round 1, on
//! the authored corpus, scored every candidate 15/15 and decided nothing;
//! `corpus-real/` is what separated them, and it reversed the ranking.
//!
//! Usage: `cargo run --release [-- CORPUS_DIR]` (default `corpus`).

use std::fs;
use std::path::PathBuf;
use std::time::Instant;

/// What the gate answers. `Unknown` is a first-class answer, not a failure:
/// for a gate it means "do not skip", which is always the safe direction.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Verdict {
    Korean,
    NotKorean,
    Unknown,
}

/// The control: no dependency. Korean iff Hangul dominates the letters.
///
/// Returns `Unknown` when the text carries neither Hangul nor ASCII letters —
/// Japanese and Chinese land here, and abstaining is the honest answer for a
/// test that can only see two scripts.
fn by_script(text: &str) -> Verdict {
    let (mut hangul, mut latin) = (0usize, 0usize);
    for c in text.chars() {
        match c {
            '\u{AC00}'..='\u{D7AF}' | '\u{1100}'..='\u{11FF}' | '\u{3130}'..='\u{318F}' => {
                hangul += 1
            }
            c if c.is_ascii_alphabetic() => latin += 1,
            _ => {}
        }
    }
    if hangul == 0 && latin == 0 {
        return Verdict::Unknown;
    }
    // 1:4 — a Korean answer stays Korean well past a heavy identifier load.
    if hangul * 4 >= latin {
        Verdict::Korean
    } else {
        Verdict::NotKorean
    }
}

fn by_whatlang(text: &str) -> Verdict {
    match whatlang::detect(text) {
        Some(info) if !info.is_reliable() => Verdict::Unknown,
        Some(info) if info.lang() == whatlang::Lang::Kor => Verdict::Korean,
        Some(_) => Verdict::NotKorean,
        None => Verdict::Unknown,
    }
}

fn by_whichlang(text: &str) -> Verdict {
    if whichlang::detect_language(text) == whichlang::Lang::Kor {
        Verdict::Korean
    } else {
        Verdict::NotKorean
    }
}

fn by_lingua(detector: &lingua::LanguageDetector, text: &str) -> Verdict {
    match detector.detect_language_of(text) {
        Some(lingua::Language::Korean) => Verdict::Korean,
        Some(_) => Verdict::NotKorean,
        None => Verdict::Unknown,
    }
}

fn cell(v: Verdict) -> &'static str {
    match v {
        Verdict::Korean => "ko",
        Verdict::NotKorean => "NOT",
        Verdict::Unknown => "?",
    }
}

/// `!` marks a wrong answer. `Unknown` against a `Korean` expectation is
/// marked wrong but is *safe*: the gate translates, costing a call it did not
/// need rather than emitting the wrong language.
fn marked(got: Verdict, want: Verdict) -> String {
    format!("{}{}", if got == want { ' ' } else { '!' }, cell(got))
}

const NAMES: [&str; 4] = ["script", "whatlang", "whichlang", "lingua"];

fn main() {
    let dir = PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| "corpus".into()));
    if !dir.is_dir() {
        eprintln!(
            "no corpus at {}\n  synthetic: python3 scripts/build-corpus-synthetic.py\n  real:      python3 scripts/build-corpus-real.py [RESP_TRANSLATOR_ROOT]",
            dir.display()
        );
        std::process::exit(2);
    }
    let detector = lingua::LanguageDetectorBuilder::from_languages(&[
        lingua::Language::Korean,
        lingua::Language::English,
    ])
    .build();
    let read = |n: &str| fs::read_to_string(dir.join(n)).ok();
    let all = |t: &str| {
        [
            by_script(t),
            by_whatlang(t),
            by_whichlang(t),
            by_lingua(&detector, t),
        ]
    };

    println!("corpus: {}\n", dir.display());

    // --- Labelled cases, when the corpus carries them -----------------------
    let mut score = [0usize; 4];
    let mut total = 0usize;
    let mut header_done = false;
    let mut labelled = |label: String, text: &str, want: Verdict, score: &mut [usize; 4], total: &mut usize| {
        if !header_done {
            println!(
                "{:<26} {:>8} {:>9} {:>10} {:>8} {:>8}",
                "case", "want", NAMES[0], NAMES[1], NAMES[2], NAMES[3]
            );
            header_done = true;
        }
        let got = all(text);
        *total += 1;
        for (i, g) in got.iter().enumerate() {
            if *g == want {
                score[i] += 1;
            }
        }
        println!(
            "{:<26} {:>8} {:>9} {:>10} {:>8} {:>8}",
            label,
            cell(want),
            marked(got[0], want),
            marked(got[1], want),
            marked(got[2], want),
            marked(got[3], want)
        );
    };

    for n in [20usize, 50, 200, 1000] {
        for kind in ["raw", "prose"] {
            if let Some(t) = read(&format!("real-ko-{kind}-{n}.txt")) {
                labelled(format!("ko {kind} {n}w"), &t, Verdict::Korean, &mut score, &mut total);
            }
        }
    }
    for (f, l) in [
        ("real-ko-full.txt", "ko FULL raw"),
        ("real-ko-full-prose.txt", "ko FULL prose"),
    ] {
        if let Some(t) = read(f) {
            labelled(l.to_string(), &t, Verdict::Korean, &mut score, &mut total);
        }
    }
    for n in [20usize, 50, 200, 1000] {
        if let Some(t) = read(&format!("real-en-{n}.txt")) {
            labelled(format!("en {n}w"), &t, Verdict::NotKorean, &mut score, &mut total);
        }
    }
    for (f, want) in [
        ("ja-200.txt", Verdict::NotKorean),
        ("zh-200.txt", Verdict::NotKorean),
        ("ko-romanized-200.txt", Verdict::Korean),
        ("en-with-ko-quote-200.txt", Verdict::NotKorean),
    ] {
        if let Some(t) = read(f) {
            labelled(f.trim_end_matches(".txt").to_string(), &t, want, &mut score, &mut total);
        }
    }
    if total > 0 {
        println!("{}", "-".repeat(74));
        println!(
            "{:<26} {:>8} {:>8}/{total} {:>8}/{total} {:>7}/{total} {:>6}/{total}",
            "correct", "", score[0], score[1], score[2], score[3]
        );
    }

    // --- The sweep that decides it -----------------------------------------
    let prefix = if dir.join("rmix000-200.txt").exists() { "rmix" } else { "mix" };
    if dir.join(format!("{prefix}000-200.txt")).exists() {
        println!("\nMIX SWEEP — % of words that are English; the rest Korean");
        println!(
            "{:<11} {:>6} {:>9} {:>10} {:>7}    {:>6} {:>9} {:>10} {:>7}",
            "", NAMES[0], NAMES[1], NAMES[2], NAMES[3], NAMES[0], NAMES[1], NAMES[2], NAMES[3]
        );
        println!("{:<11} {:^34}    {:^34}", "", "-- 200 words --", "-- 2000 words --");
        for pct in (0..=100).step_by(10) {
            let a = read(&format!("{prefix}{pct:03}-200.txt")).map(|t| all(&t));
            let b = read(&format!("{prefix}{pct:03}-2000.txt")).map(|t| all(&t));
            let f = |r: &Option<[Verdict; 4]>, i: usize| r.map_or("-", |v| cell(v[i]));
            println!(
                "{:<11} {:>6} {:>9} {:>10} {:>7}    {:>6} {:>9} {:>10} {:>7}",
                format!("{pct:>3}% en"),
                f(&a, 0), f(&a, 1), f(&a, 2), f(&a, 3),
                f(&b, 0), f(&b, 1), f(&b, 2), f(&b, 3)
            );
        }
    }

    // --- Throughput ---------------------------------------------------------
    let big = read("real-ko-full.txt")
        .or_else(|| read(&format!("{prefix}000-2000.txt")))
        .unwrap_or_default();
    if !big.is_empty() {
        const REPS: u32 = 50;
        println!("\nthroughput ({} bytes, {REPS} reps):", big.len());
        let time = |name: &str, f: &dyn Fn(&str) -> Verdict| {
            let t0 = Instant::now();
            for _ in 0..REPS {
                std::hint::black_box(f(&big));
            }
            println!(
                "  {:<10} {:>8.4} ms",
                name,
                t0.elapsed().as_secs_f64() * 1000.0 / f64::from(REPS)
            );
        };
        time(NAMES[0], &by_script);
        time(NAMES[1], &by_whatlang);
        time(NAMES[2], &by_whichlang);
        time(NAMES[3], &|t| by_lingua(&detector, t));
    }

}
