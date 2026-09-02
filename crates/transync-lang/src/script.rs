//! The free half of the gate: a script test that decides in ONE direction.
//!
//! Some languages are written in a script no other language in the backend's
//! set uses. Where that holds, counting codepoints answers the gate exactly,
//! with no model and no dependency — and the benchmark says it answers as well
//! as the library does (`benchmark/lang-detect/RESULTS.md`: 14/14 for both).
//!
//! It is deliberately not allowed to answer everything. See [`Reading`].

use crate::Language;

/// A writing system that, **within the backend's language set**, belongs to
/// exactly one language.
///
/// Exclusivity is the whole warrant. Han is absent from this list even though
/// Chinese is in the set, because Japanese is too and both write Han — a Han
/// count cannot tell them apart, so Chinese has no script shortcut and goes to
/// the backend. Kana is here because no other language in the set writes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExclusiveScript {
    Hangul,
    Kana,
    Cyrillic,
    Arabic,
    Devanagari,
}

impl ExclusiveScript {
    /// The script this target is written in, or `None` when the target shares
    /// its script with another language the backend knows.
    ///
    /// `None` is the common case and is not a defect: English, German, French,
    /// Italian, Dutch, Portuguese, Spanish, Swedish, Turkish and Vietnamese all
    /// write Latin, and Chinese shares Han with Japanese.
    pub(crate) fn of(target: Language) -> Option<Self> {
        match target {
            Language::Korean => Some(Self::Hangul),
            Language::Japanese => Some(Self::Kana),
            Language::Russian => Some(Self::Cyrillic),
            Language::Arabic => Some(Self::Arabic),
            Language::Hindi => Some(Self::Devanagari),
            _ => None,
        }
    }

    fn contains(self, c: char) -> bool {
        match self {
            // Syllables, Jamo, and compatibility Jamo.
            Self::Hangul => {
                matches!(c, '\u{AC00}'..='\u{D7AF}' | '\u{1100}'..='\u{11FF}' | '\u{3130}'..='\u{318F}')
            }
            // Hiragana and Katakana. Han is excluded on purpose — Japanese
            // shares it with Chinese, so it cannot discriminate.
            Self::Kana => matches!(c, '\u{3040}'..='\u{309F}' | '\u{30A0}'..='\u{30FF}'),
            Self::Cyrillic => matches!(c, '\u{0400}'..='\u{04FF}' | '\u{0500}'..='\u{052F}'),
            Self::Arabic => matches!(c, '\u{0600}'..='\u{06FF}' | '\u{0750}'..='\u{077F}'),
            Self::Devanagari => matches!(c, '\u{0900}'..='\u{097F}'),
        }
    }
}

/// What one pass over the text found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Reading {
    /// The target's own script carries at least [`FLOOR_DENOMINATOR`]⁻¹ of the
    /// letters. Decided here; the backend is not consulted.
    Dominant,
    /// Not one character of the target's script, but the text does have
    /// letters. Decided here.
    Absent,
    /// The script is present but a minority. **Undecidable by counting** — this
    /// is the band where the library earns its keep, so it is handed on.
    Minority,
    /// No letters at all: punctuation, digits, code, or nothing.
    NoLetters,
}

/// The target's script must be at least one letter in `FLOOR_DENOMINATOR` for
/// [`Reading::Dominant`].
///
/// Five, i.e. 20 %, and it is measured rather than chosen for roundness. An
/// agent explaining code in Korean emits a heavy load of English identifiers,
/// and that answer is still Korean; the gate has to keep saying so. On the real
/// corpus this threshold holds "already Korean" until 60 % of the words are
/// English (`benchmark/lang-detect/RESULTS.md`). It is a private constant, not
/// a knob: across the measured sweep every multiplier from 1 to 9 produces
/// identical verdicts, so a caller-tunable ratio would have no reachable effect
/// except to contradict the evidence.
const FLOOR_DENOMINATOR: usize = 5;

/// Count the target's script against every other letter, in one pass.
///
/// "Other letter" is `char::is_alphabetic`, not ASCII: a Korean answer quoting
/// Greek or Cyrillic should dilute the ratio exactly as English does.
pub(crate) fn read(text: &str, script: ExclusiveScript) -> Reading {
    let (mut in_script, mut other) = (0usize, 0usize);
    for c in text.chars() {
        if script.contains(c) {
            in_script += 1;
        } else if c.is_alphabetic() {
            other += 1;
        }
    }
    let letters = in_script + other;
    if letters == 0 {
        return Reading::NoLetters;
    }
    if in_script == 0 {
        return Reading::Absent;
    }
    if in_script * FLOOR_DENOMINATOR >= letters {
        Reading::Dominant
    } else {
        Reading::Minority
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The boundary, both sides, with the arithmetic written out rather than
    /// trusted: 20 of 100 letters is dominant, 19 is not.
    #[test]
    fn the_floor_is_exactly_one_letter_in_five() {
        let at = "가".repeat(20) + &"a".repeat(80);
        let below = "가".repeat(19) + &"a".repeat(81);
        assert_eq!(read(&at, ExclusiveScript::Hangul), Reading::Dominant);
        assert_eq!(read(&below, ExclusiveScript::Hangul), Reading::Minority);
    }

    #[test]
    fn absent_and_no_letters_are_different_answers() {
        assert_eq!(
            read("hello world", ExclusiveScript::Hangul),
            Reading::Absent
        );
        assert_eq!(read("", ExclusiveScript::Hangul), Reading::NoLetters);
        assert_eq!(
            read("{ } ; 42 // 3.14", ExclusiveScript::Hangul),
            Reading::NoLetters
        );
    }

    /// Non-ASCII letters dilute too. A Korean sentence quoting a long Greek
    /// passage is not confidently Korean, and counting only ASCII would have
    /// called it so.
    #[test]
    fn every_alphabetic_char_counts_against_the_target_script() {
        let greek = "가".repeat(10) + &"λ".repeat(90);
        assert_eq!(read(&greek, ExclusiveScript::Hangul), Reading::Minority);
    }

    /// Han is not kana. Chinese and Japanese both write Han, so counting it
    /// would make the Japanese shortcut answer for Chinese text.
    #[test]
    fn han_is_not_claimed_by_the_japanese_shortcut() {
        assert_eq!(read("漢字漢字漢字", ExclusiveScript::Kana), Reading::Absent);
        assert_eq!(read("ひらがな", ExclusiveScript::Kana), Reading::Dominant);
    }

    /// Every language whose script another language in the set also writes must
    /// answer `None`, or the shortcut would decide a question it cannot see.
    #[test]
    fn only_languages_with_an_unshared_script_get_a_shortcut() {
        for shared in [
            Language::English,
            Language::German,
            Language::French,
            Language::Italian,
            Language::Dutch,
            Language::Portuguese,
            Language::Spanish,
            Language::Swedish,
            Language::Turkish,
            Language::Vietnamese,
            // Han, shared with Japanese.
            Language::Chinese,
        ] {
            assert!(
                ExclusiveScript::of(shared).is_none(),
                "{shared:?} shares its script and must reach the backend"
            );
        }
        assert!(ExclusiveScript::of(Language::Korean).is_some());
    }
}
