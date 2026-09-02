//! The **only** file in this crate that names `whichlang`.
//!
//! That containment is a semver decision, not tidiness. `whichlang` is 0.1.x,
//! so under Cargo's rules every one of its releases may break: if
//! `whichlang::Lang` reached this crate's public API, their next `0.2` would be
//! our next major, and a consumer would have to match on a type we do not own.
//! `crate::containment` asserts the containment by scraping `src/`, so the day
//! someone adds a second `use whichlang` the suite says so.
//!
//! The mapping below is a **wildcard-free match**. That is the second half of
//! the containment: when the backend adds a language, this file stops
//! compiling, which is exactly where the decision belongs — a new variant needs
//! a name in [`Language`] and a considered answer, not a silent fall-through to
//! "not the target".

use crate::Language;

/// Ask the backend what language `text` is, and translate its answer into ours.
///
/// `None` means the backend named a language this crate does not model. That
/// cannot happen today — the match is exhaustive over its enum — and it is
/// still an `Option` because "the backend grew a variant" is a state a caller
/// must be able to be told about rather than have guessed at.
pub(crate) fn detect(text: &str) -> Option<Language> {
    Some(match whichlang::detect_language(text) {
        whichlang::Lang::Ara => Language::Arabic,
        whichlang::Lang::Cmn => Language::Chinese,
        whichlang::Lang::Deu => Language::German,
        whichlang::Lang::Eng => Language::English,
        whichlang::Lang::Fra => Language::French,
        whichlang::Lang::Hin => Language::Hindi,
        whichlang::Lang::Ita => Language::Italian,
        whichlang::Lang::Jpn => Language::Japanese,
        whichlang::Lang::Kor => Language::Korean,
        whichlang::Lang::Nld => Language::Dutch,
        whichlang::Lang::Por => Language::Portuguese,
        whichlang::Lang::Rus => Language::Russian,
        whichlang::Lang::Spa => Language::Spanish,
        whichlang::Lang::Swe => Language::Swedish,
        whichlang::Lang::Tur => Language::Turkish,
        whichlang::Lang::Vie => Language::Vietnamese,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The backend is total and never panics on the inputs the gate can hand
    /// it. It is only ever called with letters present (`lib.rs` returns before
    /// this on `NoLetters`), because `detect_language` answers `Eng` for text
    /// with no features at all — a default that would read as a real English
    /// verdict here.
    #[test]
    fn the_mapping_answers_for_ordinary_text_in_both_directions() {
        assert_eq!(
            detect("This is an ordinary English sentence about software."),
            Some(Language::English)
        );
        assert_eq!(
            detect("이 문장은 한국어로 작성된 평범한 문장입니다."),
            Some(Language::Korean)
        );
    }

    /// Pinned so the empty-input default is a fact of record rather than
    /// folklore: it is why `lib.rs` must screen `NoLetters` before calling here.
    #[test]
    fn the_backend_defaults_to_english_on_featureless_input() {
        assert_eq!(detect(""), Some(Language::English));
    }
}
