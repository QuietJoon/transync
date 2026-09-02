//! The source-language **gate**: *should a translation run start at all?*
//!
//! A consumer captures a piece of text — an agent's final answer, a pasted
//! document — and wants to skip the provider call when it is already in the
//! target language. Answering that wrongly in the expensive direction, calling
//! genuine Korean "not Korean", costs a call **and** emits ko→ko output.
//!
//! ```
//! use transync_lang::{Gate, Language, Verdict};
//!
//! let gate = Gate::new(Language::Korean);
//! match gate.verdict("이 문장은 이미 한국어입니다.") {
//!     Verdict::AlreadyTarget(_) => { /* skip: nothing to translate */ }
//!     Verdict::Translate(_) => { /* run the pipeline */ }
//! }
//! ```
//!
//! # This is not the pipeline's language answer, and must never become it
//!
//! transync already answers *"what language is this document"*: the alignment
//! map's `detected_source_language`, authored by the **provider** under
//! `--source-language auto` and admitted through one door in the pipeline. That
//! value is the model's observation about text it actually translated; it is on
//! the wire, in the cache, and on the rendered page.
//!
//! This crate answers a different question, for a different party, at a
//! different time: **before** any run, for the **caller**, "is there anything to
//! translate?" The two must not converge. One question with two answers that can
//! disagree is the defect class this workspace spent 2026-09-01 removing — a
//! second opinion about where a tag name ends (ti `415cdb`, ti `e20490`), about
//! where foreign content begins (ti `2e2453`), about which blocks anchor
//! (ti `18b9c3`). A local guess that could be written into
//! `detected_source_language` would be the same shape one layer up.
//!
//! Three things enforce the separation rather than merely asking for it:
//! [`Verdict`] carries **no language name**, so there is nothing to copy into
//! that field; it implements no serialization, so it cannot reach a wire format;
//! and this crate depends on no workspace member, so no engine crate can call it
//! without an edge someone has to add on purpose.
//!
//! # How it decides
//!
//! Script first, backend second — and the script test is only allowed to answer
//! where counting is conclusive. See [`Gate::verdict`]. The composition is
//! measured, not assumed: `benchmark/lang-detect/RESULTS.md` records the
//! comparison that chose the backend and the threshold, including the
//! uncomfortable part — a dependency-free codepoint test ties the winner on
//! real fixtures and is ten times faster, which is why it runs first.
//!
//! TRACE: ti e4f4b0
//! TRACE: benchmark/lang-detect/RESULTS.md

mod backend;
// Test-only scaffolding: the weld that keeps `whichlang` inside `backend`.
// Declared under `cfg(test)` so it is scaffolding by construction rather
// than by convention — it owns no state and ships nothing.
#[cfg(test)]
mod containment;
mod script;

use script::{ExclusiveScript, Reading};

/// A language this gate can reason about.
///
/// Exactly the set the backend recognizes, named in full rather than by code.
/// `#[non_exhaustive]` because that set can grow: a consumer must not write a
/// `match` that a new language silently falls out of.
///
/// This is deliberately **our** enum and not the backend's. `whichlang` is
/// 0.1.x, where every release may break; re-exporting its type would make its
/// semver ours and hand consumers a type we do not control.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Language {
    Arabic,
    Chinese,
    Dutch,
    English,
    French,
    German,
    Hindi,
    Italian,
    Japanese,
    Korean,
    Portuguese,
    Russian,
    Spanish,
    Swedish,
    Turkish,
    Vietnamese,
}

impl Language {
    /// The ISO 639-3 code, for a caller that has to name this on a wire it
    /// already owns.
    pub fn iso639_3(self) -> &'static str {
        match self {
            Self::Arabic => "ara",
            Self::Chinese => "cmn",
            Self::Dutch => "nld",
            Self::English => "eng",
            Self::French => "fra",
            Self::German => "deu",
            Self::Hindi => "hin",
            Self::Italian => "ita",
            Self::Japanese => "jpn",
            Self::Korean => "kor",
            Self::Portuguese => "por",
            Self::Russian => "rus",
            Self::Spanish => "spa",
            Self::Swedish => "swe",
            Self::Turkish => "tur",
            Self::Vietnamese => "vie",
        }
    }

    /// Parse an ISO 639-1 or 639-3 code, ASCII-case-insensitively.
    ///
    /// **Strict on purpose.** No region subtags, no aliases, no best-effort
    /// guess: `"ko"` and `"kor"` resolve, `"ko-KR"` does not. A lenient parse
    /// fails in the unsafe direction — it would let a typo resolve to a
    /// language the caller did not mean, and the gate would then skip a
    /// translation on the strength of a misreading.
    pub fn from_iso639(code: &str) -> Option<Self> {
        const TABLE: &[(&str, &str, Language)] = &[
            ("ar", "ara", Language::Arabic),
            ("zh", "cmn", Language::Chinese),
            ("nl", "nld", Language::Dutch),
            ("en", "eng", Language::English),
            ("fr", "fra", Language::French),
            ("de", "deu", Language::German),
            ("hi", "hin", Language::Hindi),
            ("it", "ita", Language::Italian),
            ("ja", "jpn", Language::Japanese),
            ("ko", "kor", Language::Korean),
            ("pt", "por", Language::Portuguese),
            ("ru", "rus", Language::Russian),
            ("es", "spa", Language::Spanish),
            ("sv", "swe", Language::Swedish),
            ("tr", "tur", Language::Turkish),
            ("vi", "vie", Language::Vietnamese),
        ];
        TABLE
            .iter()
            .find(|(two, three, _)| {
                code.eq_ignore_ascii_case(two) || code.eq_ignore_ascii_case(three)
            })
            .map(|(_, _, lang)| *lang)
    }
}

/// How the gate reached [`Verdict::AlreadyTarget`].
///
/// Reporting only; the caller's action is the same either way. It exists so a
/// log can say which half of the gate answered without the caller re-deriving it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Basis {
    /// The target's own script carried at least a fifth of the letters, and no
    /// other language in the backend's set writes that script.
    ExclusiveScript,
    /// The backend named the target.
    Backend,
}

/// Why the gate returned [`Verdict::Translate`].
///
/// Note that "could not tell" lives **here**, among the reasons to proceed, and
/// not in a third variant. That placement is the safety property: see [`Verdict`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Reason {
    /// Not one character of the target's exclusive script, though the text has
    /// letters. Conclusive without the backend.
    ExclusiveScriptAbsent,
    /// The backend named some other language.
    BackendDiffers,
    /// **The gate does not know.** No letters to judge at all — punctuation,
    /// digits, bare code, or empty. Translating is the safe response to
    /// ignorance, which is why this is a reason to proceed rather than an
    /// outcome of its own.
    Undecided,
}

/// What the gate concluded.
///
/// **Two variants, exhaustive, and the safe direction is structural.** A caller
/// writes `match` and gets exactly two arms; the only way to skip a translation
/// is to name [`Verdict::AlreadyTarget`] explicitly.
///
/// That shape is chosen against the obvious alternatives. A flat
/// `{ AlreadyTarget, Differs, Unknown }` makes it possible to skip on `Unknown`
/// by writing one careless arm, and `Option<bool>` makes it possible with
/// `unwrap_or(true)` — both put the expensive mistake one keystroke away.
/// Here "I could not tell" is a [`Reason`] *inside* `Translate`, so the gate
/// cannot be misread into skipping when it does not know.
///
/// Deliberately **not** `#[non_exhaustive]`: a third outcome would land behind
/// consumers' `_` arms, and whichever direction their wildcard chose would
/// silently become the gate's policy. If a third outcome is ever right, it
/// should break every consumer's build and be decided at each site.
///
/// It carries no [`Language`] and derives no serialization, so it cannot become
/// the alignment map's `detected_source_language`. See the crate docs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// The text is already in the target language; the caller may skip.
    AlreadyTarget(Basis),
    /// Proceed with the translation — including because the gate could not tell.
    Translate(Reason),
}

impl Verdict {
    /// `true` when the caller should run the translation.
    ///
    /// A convenience for call sites that only want the bit. It is not the
    /// safety mechanism — [`Verdict`]'s own shape is — so reaching for `match`
    /// instead costs nothing.
    pub fn should_translate(self) -> bool {
        matches!(self, Self::Translate(_))
    }
}

/// The gate, built once around a target language and reused.
///
/// `#[non_exhaustive]` so a future knob can arrive as a `with_*` builder method
/// without breaking construction.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct Gate {
    target: Language,
}

impl Gate {
    /// Build a gate for `target`.
    pub fn new(target: Language) -> Self {
        Self { target }
    }

    /// The language this gate was built for.
    pub fn target(&self) -> Language {
        self.target
    }

    /// Decide whether `text` still needs translating into the gate's target.
    ///
    /// The order is the measured one — the free test first, and only where it
    /// is conclusive:
    ///
    /// 1. **No letters at all** → [`Reason::Undecided`]. This screen comes
    ///    first and matters: the backend answers `English` for featureless
    ///    input, and that default would otherwise read as a real verdict.
    /// 2. **Target has an exclusive script** (Korean, Japanese, Russian,
    ///    Arabic, Hindi — see `script`):
    ///    - at least a fifth of the letters in that script →
    ///      [`Basis::ExclusiveScript`];
    ///    - none of it, with other letters present →
    ///      [`Reason::ExclusiveScriptAbsent`];
    ///    - present but a minority → **hand to the backend**. This band is the
    ///      only place the library beat the ratio in measurement (it holds
    ///      "already target" to 70 % dilution where counting gives up at 60 %),
    ///      so it is the only place the library is asked.
    /// 3. **Otherwise** → the backend, and its answer compared to the target.
    ///
    /// A target whose script another language in the set also writes — every
    /// Latin one, and Chinese, which shares Han with Japanese — skips step 2
    /// entirely rather than counting a script that cannot discriminate.
    ///
    /// Only Korean's threshold is **measured**
    /// (`benchmark/lang-detect/RESULTS.md`). The other four exclusive scripts
    /// take the same rule by construction — a script no other candidate writes
    /// — and their dilution behaviour has not been benchmarked.
    pub fn verdict(&self, text: &str) -> Verdict {
        if let Some(script) = ExclusiveScript::of(self.target) {
            match script::read(text, script) {
                Reading::NoLetters => return Verdict::Translate(Reason::Undecided),
                Reading::Dominant => return Verdict::AlreadyTarget(Basis::ExclusiveScript),
                Reading::Absent => return Verdict::Translate(Reason::ExclusiveScriptAbsent),
                // Present but a minority: exactly the band the backend is for.
                Reading::Minority => {}
            }
        } else if !text.chars().any(char::is_alphabetic) {
            return Verdict::Translate(Reason::Undecided);
        }

        match backend::detect(text) {
            Some(found) if found == self.target => Verdict::AlreadyTarget(Basis::Backend),
            Some(_) => Verdict::Translate(Reason::BackendDiffers),
            // The backend grew a language we do not model. Not knowing is a
            // reason to translate, never to skip.
            None => Verdict::Translate(Reason::Undecided),
        }
    }
}
