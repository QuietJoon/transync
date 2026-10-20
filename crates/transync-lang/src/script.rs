//! The free half of the gate: a script test that decides in ONE direction.
//!
//! Some languages are written in a script no other language in the backend's
//! set uses. Where that holds, counting codepoints answers the gate exactly,
//! with no model and no dependency — and the benchmark says it answers as well
//! as the library does (`benchmark/lang-detect/RESULTS.md`: 14/14 for both).
//!
//! "No other language in the backend's set" is not "no other language". Where
//! another orthography writes the same script with letters the target never
//! uses, one such letter settles the question the other way before the count
//! is read ([`ExclusiveScript::foreign`], R0012-0014, DCR-0063).
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
///
/// The warrant stops at the set's edge. Cyrillic, the Arabic script and
/// Devanagari are each written by many languages outside it, so those three
/// carry a screen for the letters their one language never uses
/// ([`Self::foreign`]). Hangul and Kana carry none: they are near-exclusive
/// rather than exclusive (Jeju writes Hangul; Ainu and the Ryukyuan languages
/// write kana), and the crate documents that as outside its input domain.
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

    /// A letter of this script that the one language in the backend's set
    /// writing it never uses (R0012-0014, DCR-0063).
    ///
    /// Only asked of a letter [`Self::contains`] has already placed in the
    /// script. One such letter is positive evidence of another orthography, and
    /// [`read`] answers [`Reading::Foreign`] on it before the ratio is looked
    /// at. Neither half of the gate can see that difference otherwise: the
    /// count treats every letter in the block alike, and the backend sees only
    /// 128-codepoint block buckets outside ASCII, so it names Russian for
    /// Ukrainian, Arabic for Persian and Hindi for Marathi.
    ///
    /// - **Cyrillic**: every letter outside the Russian alphabet's 33, in either
    ///   case. That covers Ukrainian і ї є ґ, Belarusian ў, Serbian and
    ///   Macedonian ј љ њ ћ ђ џ ѕ ѓ ќ, and Kazakh, Kyrgyz, Tajik and Mongolian
    ///   ә ғ қ ң ө ұ ү һ ҷ ҳ ӯ ӣ, among others. Bulgarian writes a subset of the
    ///   Russian alphabet and is **not** caught.
    /// - **Arabic script**: the letters the Arabic alphabet does not use — every
    ///   letter of the Arabic block outside it (U+0620, U+063B–U+063F,
    ///   U+0672–U+06D3, U+06D5, U+06EE–U+06EF, U+06FA–U+06FC, U+06FF: Persian
    ///   and Urdu پ چ ژ گ ک ی ٹ ڈ ڑ ں ے, Pashto ټ ډ ړ ښ ګ ڼ ې ۍ, Kurdish
    ///   ڕ ڵ ۆ ێ ە, Jawi ڠ ڤ, among others) and the whole Arabic Supplement
    ///   block. Harakat, Quranic marks, tatweel, alef wasla and the dotless
    ///   archaic letters are Arabic and stay out.
    /// - **Devanagari**: the independent letters Hindi's orthography never
    ///   writes — ळ (Marathi, Rajasthani), ॲ and ऱ (Marathi), ऩ and ऴ
    ///   (Dravidian transliteration), the short ऄ ऎ ऒ, Sanskrit's vocalic
    ///   ऌ ॠ ॡ, and the U+0973–U+097F letters of Kashmiri, Marwari, Sindhi and
    ///   others. Nepali and Sanskrit, and Marathi that happens to carry none of
    ///   these, are **not** caught.
    ///
    /// The cost lands in the safe direction: a target-language text quoting one
    /// such word is sent for translation.
    pub(crate) fn foreign(self, c: char) -> bool {
        match self {
            Self::Hangul | Self::Kana => false,
            Self::Cyrillic => !matches!(c, '\u{0410}'..='\u{044F}' | '\u{0401}' | '\u{0451}'),
            Self::Arabic => matches!(
                c,
                '\u{0620}'
                    | '\u{063B}'..='\u{063F}'
                    | '\u{0672}'..='\u{06D3}'
                    | '\u{06D5}'
                    | '\u{06EE}'..='\u{06EF}'
                    | '\u{06FA}'..='\u{06FC}'
                    | '\u{06FF}'
                    | '\u{0750}'..='\u{077F}'
            ),
            Self::Devanagari => matches!(
                c,
                '\u{0904}'
                    | '\u{090C}'
                    | '\u{090E}'
                    | '\u{0912}'
                    | '\u{0929}'
                    | '\u{0931}'
                    | '\u{0933}'
                    | '\u{0934}'
                    | '\u{0960}'
                    | '\u{0961}'
                    | '\u{0972}'..='\u{097F}'
            ),
        }
    }
}

/// What one pass over the text found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Reading {
    /// At least one letter of the target's script that the target's
    /// orthography never uses ([`ExclusiveScript::foreign`]): another
    /// language writing the same script. Decided here **whatever the ratio**,
    /// so it settles the text before [`Reading::Dominant`] and before the
    /// [`Reading::Minority`] hand-off alike; the backend could not tell the
    /// difference either.
    Foreign,
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
///
/// **Only letters are counted at all**, on either side. A codepoint that is
/// not `char::is_alphabetic` is skipped even when it sits inside the target's
/// own Unicode block, because a block is not a letter set — see the comment in
/// the loop for the digits and punctuation this excludes, and R0010-0066 for
/// the verdict it used to produce. This is what makes [`Reading::NoLetters`]'
/// "punctuation, digits, code, or nothing" true for every script rather than
/// only for the ones whose blocks happen to be pure `Lo`.
///
/// The first letter [`ExclusiveScript::foreign`] flags ends the pass with
/// [`Reading::Foreign`]: no count can outweigh it, so none is finished.
pub(crate) fn read(text: &str, script: ExclusiveScript) -> Reading {
    let (mut in_script, mut other) = (0usize, 0usize);
    for c in text.chars() {
        // Alphabeticity is asked FIRST, of both sides (R0010-0066). A script
        // BLOCK is not a letter set: the Arabic ranges carry Arabic-Indic
        // digits and their own comma, semicolon and question mark, Devanagari
        // carries its digits and both dandas, Kana carries the middle dot.
        // Testing block membership first counted every one of those as a
        // target-script letter, which is why `Gate::new(Language::Arabic)`
        // called "Order ١٢٣٤٥٦ shipped" already-Arabic.
        //
        // That is the one direction this crate defines as unsafe: naming
        // `AlreadyTarget` is the only way a run is skipped, so an over-count
        // here does not degrade a translation, it cancels one.
        //
        // `is_alphabetic` is Unicode `Alphabetic`, not `L*`, so the modifier
        // letters and combining marks these scripts actually need — Kana's
        // prolonged sound mark, Arabic tatweel and harakat — keep counting.
        if !c.is_alphabetic() {
            continue;
        }
        if script.contains(c) {
            // R0012-0014: exclusive within the backend's set is not exclusive.
            // A letter the target never writes names another orthography,
            // and the ratio below cannot see it.
            if script.foreign(c) {
                return Reading::Foreign;
            }
            in_script += 1;
        } else {
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
        // R0012-0014 keeps the shortcut for the three shared-worldwide scripts
        // and screens them instead (`foreign`); it does not withdraw them.
        for screened in [Language::Russian, Language::Arabic, Language::Hindi] {
            assert!(
                ExclusiveScript::of(screened).is_some(),
                "{screened:?} keeps its script shortcut"
            );
        }
    }

    /// R0012-0014. Every letter the owner's decision named, one at a time, in
    /// otherwise genuine target text, reads as another orthography. Upper case
    /// too where the script has case. The last two rows are the rest of the
    /// screen as `foreign` documents it: Sindhi swash kaf and beeh, Jawi gaf
    /// from Arabic Supplement, and Devanagari's other non-Hindi letters.
    #[test]
    fn every_named_foreign_letter_is_read_as_another_orthography() {
        let cases: [(ExclusiveScript, &str, &str); 6] = [
            (
                ExclusiveScript::Cyrillic,
                "Это русский текст ",
                "іїєґўјљњћђџѕѓќәғқңөұүһҷҳӯӣ",
            ),
            (
                ExclusiveScript::Cyrillic,
                "Это русский текст ",
                "ІЇЄҐЎЈЉЊЋЂЏЅЃЌӘҒҚҢӨҰҮҺҶҲӮӢ",
            ),
            (
                ExclusiveScript::Arabic,
                "هذا نص عربي ",
                "پچژگکیٹڈڑںےټډړښګڼېۍڕڵۆێەڠڤ",
            ),
            (ExclusiveScript::Devanagari, "यह हिंदी पाठ है ", "ळ"),
            (
                ExclusiveScript::Arabic,
                "هذا نص عربي ",
                "\u{06AA}\u{067B}\u{0762}",
            ),
            (
                ExclusiveScript::Devanagari,
                "यह हिंदी पाठ है ",
                "ॲऱऩऴऄऎऒऌॠॡ\u{097B}",
            ),
        ];
        let mut missed = Vec::new();
        for (script, genuine, letters) in cases {
            assert_ne!(
                read(genuine, script),
                Reading::Foreign,
                "the control {genuine:?} must be clean"
            );
            for c in letters.chars() {
                if read(&format!("{genuine}{c}"), script) != Reading::Foreign {
                    missed.push(format!("{script:?} U+{:04X} {c}", c as u32));
                }
            }
        }
        assert!(
            missed.is_empty(),
            "letters the target never uses: {missed:?}"
        );
    }

    /// The other side of the screen, enumerated rather than sampled: no letter
    /// the target's own orthography writes may trip it, or genuine target text
    /// would lose its shortcut.
    #[test]
    fn the_targets_own_letters_are_never_foreign() {
        let russian = ('\u{0410}'..='\u{044F}').chain(['\u{0401}', '\u{0451}']);
        // Hamza through ghain, tatweel through yeh, the harakat with maddah
        // and both hamza marks, subscript and superscript alef, alef wasla,
        // dotless beh and qaf, the honorific signs, and the Quranic
        // annotation marks (small waw and yeh among them).
        let arabic = ('\u{0621}'..='\u{063A}')
            .chain('\u{0640}'..='\u{0656}')
            .chain('\u{0610}'..='\u{061A}')
            .chain(['\u{066E}', '\u{066F}', '\u{0670}', '\u{0671}'])
            .chain('\u{06D6}'..='\u{06ED}');
        // Hindi's vowels (with candra E and O for loans), consonants, the
        // nukta consonants, the bindus and visarga, nukta, avagraha, its
        // matras, virama, and om.
        let hindi = ('\u{0900}'..='\u{0903}')
            .chain('\u{0905}'..='\u{090B}')
            .chain(['\u{090D}', '\u{090F}', '\u{0910}', '\u{0911}'])
            .chain(['\u{0913}', '\u{0914}'])
            .chain('\u{0915}'..='\u{0928}')
            .chain('\u{092A}'..='\u{0930}')
            .chain(['\u{0932}'])
            .chain('\u{0935}'..='\u{0939}')
            .chain('\u{093C}'..='\u{0945}')
            .chain('\u{0947}'..='\u{0949}')
            .chain('\u{094B}'..='\u{094D}')
            .chain(['\u{0950}'])
            .chain('\u{0958}'..='\u{095F}');
        let mut tripped = Vec::new();
        for (script, letters) in [
            (ExclusiveScript::Cyrillic, russian.collect::<Vec<_>>()),
            (ExclusiveScript::Arabic, arabic.collect()),
            (ExclusiveScript::Devanagari, hindi.collect()),
        ] {
            for c in letters {
                assert!(script.contains(c), "U+{:04X} is in the block", c as u32);
                if script.foreign(c) {
                    tripped.push(format!("{script:?} U+{:04X} {c}", c as u32));
                }
            }
        }
        assert!(tripped.is_empty(), "the target writes these: {tripped:?}");
    }

    /// The screen is decided whatever the ratio: one foreign letter among a
    /// majority of the target's own (where `Dominant` used to answer) and
    /// among a minority of it (where the backend used to be asked) alike.
    #[test]
    fn a_foreign_letter_decides_in_both_bands() {
        let dominant = "русский".repeat(10) + "ї";
        let minority = "ї".to_string() + &"a".repeat(100);
        assert_eq!(read(&dominant, ExclusiveScript::Cyrillic), Reading::Foreign);
        assert_eq!(read(&minority, ExclusiveScript::Cyrillic), Reading::Foreign);
        // A Latin-script text quoting no Cyrillic at all stays `Absent`.
        assert_eq!(
            read("plain English", ExclusiveScript::Cyrillic),
            Reading::Absent
        );
    }

    /// Hangul and Kana carry no screen. Their near-exclusivity is stated as
    /// the gate's input domain instead (DCR-0063), and this pins the statement:
    /// Jeju's arae-a (U+119E, in conjoining jamo) reads as Hangul like any
    /// other syllable.
    #[test]
    fn hangul_and_kana_carry_no_screen() {
        for c in ['가', '\u{119E}', '\u{318D}', 'あ', 'ア'] {
            assert!(!ExclusiveScript::Hangul.foreign(c));
            assert!(!ExclusiveScript::Kana.foreign(c));
        }
        assert_eq!(
            read("\u{1106}\u{119E}\u{11AF}", ExclusiveScript::Hangul),
            Reading::Dominant
        );
    }

    /// R0010-0066. Membership in a script BLOCK is not the same question as
    /// being a letter, and this module's own vocabulary already says so:
    /// [`Reading::NoLetters`] means "punctuation, digits, code, or nothing",
    /// and [`read`]'s contract is to count the target's script "against every
    /// other **letter**". Unicode blocks are not letter sets — the Arabic
    /// ranges carry Arabic-Indic digits and their own comma, semicolon and
    /// question mark; Devanagari carries its digits and both dandas; Kana
    /// carries the middle dot.
    ///
    /// The damage is one-directional and it is the direction the crate defines
    /// as unsafe: naming `AlreadyTarget` is the only way a run is skipped
    /// entirely, so counting a digit as a target-script letter can make English
    /// prose read as already-translated. Hangul cannot show it — all three of
    /// its ranges are `Lo` — which is why the measured Korean corpus never did.
    #[test]
    fn digits_and_punctuation_inside_a_script_block_are_not_letters() {
        // Six Arabic-Indic digits, twelve ASCII letters, and not one Arabic
        // letter. Before the fix: in_script 6, other 12, 6 * 5 >= 18 ->
        // Dominant, i.e. "this English sentence is already Arabic".
        assert_eq!(
            read(
                "Order \u{0661}\u{0662}\u{0663}\u{0664}\u{0665}\u{0666} shipped",
                ExclusiveScript::Arabic
            ),
            Reading::Absent,
            "Arabic-Indic digits are digits; the sentence carries no Arabic letter"
        );
        // Devanagari digits, same shape.
        assert_eq!(
            read(
                "Total \u{0967}\u{0968}\u{0969} items",
                ExclusiveScript::Devanagari
            ),
            Reading::Absent,
            "Devanagari digits are digits"
        );
        // Script-block punctuation alone is still NoLetters, exactly as the
        // ASCII case above already asserts for Hangul.
        assert_eq!(
            read("\u{060C}\u{061B}\u{061F}", ExclusiveScript::Arabic),
            Reading::NoLetters,
            "Arabic comma, semicolon and question mark are punctuation"
        );
        assert_eq!(
            read("\u{0964}\u{0965}", ExclusiveScript::Devanagari),
            Reading::NoLetters,
            "the dandas are punctuation"
        );
        assert_eq!(
            read("\u{30FB}", ExclusiveScript::Kana),
            Reading::NoLetters,
            "the katakana middle dot is punctuation"
        );
    }

    /// The other half of the same rule: a real letter must keep counting.
    /// `is_alphabetic` is `Alphabetic`, not `L*`, so the modifier letters and
    /// combining marks these scripts genuinely need are still letters — the
    /// fix must not buy correctness on digits by dropping them.
    #[test]
    fn script_letters_that_are_not_l_still_count() {
        // U+30FC KATAKANA-HIRAGANA PROLONGED SOUND MARK is Lm.
        assert_eq!(
            read("\u{30B3}\u{30FC}\u{30C9}", ExclusiveScript::Kana),
            Reading::Dominant,
            "the prolonged sound mark is a letter and is inside the block"
        );
        // U+0640 ARABIC TATWEEL is Lm; U+064E FATHA is Other_Alphabetic.
        assert_eq!(
            read("\u{0645}\u{0640}\u{064E}", ExclusiveScript::Arabic),
            Reading::Dominant,
            "tatweel and harakat are alphabetic and stay in the count"
        );
    }

    /// Hangul is all `Lo` in every range this crate names, so the fix is a
    /// provable no-op for the one target the benchmark corpus measured
    /// (`benchmark/lang-detect/RESULTS.md`). Pinned so a later widening of
    /// `ExclusiveScript::Hangul` has to face this.
    #[test]
    fn the_measured_korean_behaviour_is_unchanged() {
        let at = "가".repeat(20) + &"a".repeat(80);
        assert_eq!(read(&at, ExclusiveScript::Hangul), Reading::Dominant);
        assert_eq!(
            read("한국어 문서입니다", ExclusiveScript::Hangul),
            Reading::Dominant
        );
        assert_eq!(
            read("숫자 123 과 기호 !? 포함", ExclusiveScript::Hangul),
            Reading::Dominant,
            "ASCII digits were never counted; this is the control"
        );
    }
}
