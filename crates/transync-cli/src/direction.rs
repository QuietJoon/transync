//! Bundle text direction (OI-0032).
//!
//! The rendered bundle stamps `dir` at the **pane** level: `dir` inherits, and
//! there is no per-block language metadata to key a finer-grained decision on
//! (the alignment map is presentation-free by contract). The sync engine is
//! unaffected — it works in vertical `offsetTop`/`offsetHeight` geometry, which
//! is direction-agnostic, so `sync.js` needs no change.
//!
//! Resolution order: an explicit `--target-direction` flag wins, then the
//! profile's `[render].target_direction`, then the built-in `auto`. `auto`
//! applies a small built-in RTL primary-subtag table to the language label.
//! That is a **presentation hint only**: ADR-0013's opaque labels stay opaque
//! for the prompt, the cache key, the alignment map, and validation — a label
//! that is a language *name* rather than a tag simply misses the table and
//! renders LTR, and the explicit flag/profile key is the escape hatch.
//!
//! TRACE: ADR-0013
//! TRACE: ADR-0006

/// Values accepted by `--target-direction` and by the profile's
/// `[render].target_direction`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum DirectionMode {
    /// Resolve from the language label's primary subtag using the built-in
    /// RTL table (see `RTL_PRIMARY_SUBTAGS`).
    Auto,
    /// Force left-to-right, which emits no attribute at all — left-to-right
    /// is the HTML default.
    Ltr,
    /// Force right-to-left.
    Rtl,
}

/// The one attribute this module ever emits. A fixed literal, so no
/// caller-supplied text can reach the generated HTML through it.
const RTL_ATTR: &str = " dir=\"rtl\"";

/// Primary language subtags whose scripts are written right-to-left.
///
/// Best-effort and deliberately small — this is a default-picking hint, not a
/// language database. Includes the BCP-47 grandfathered legacy codes `iw`
/// (→ `he`) and `ji` (→ `yi`) because real-world labels still carry them.
/// `ku` is deliberately absent: Kurmanji Kurdish is Latin-script, and the
/// right-to-left Sorani variant has its own subtag (`ckb`).
const RTL_PRIMARY_SUBTAGS: &[&str] = &[
    "ar", "arc", "ckb", "dv", "fa", "he", "iw", "ji", "nqo", "ps", "sd", "syr", "ug", "ur", "yi",
];

/// The `dir` attribute for one pane: `" dir=\"rtl\""` when the pane resolves to
/// right-to-left, otherwise the empty string.
///
/// LTR emits **nothing** rather than `dir="ltr"`: left-to-right is the HTML
/// default, and the absence keeps every existing (ko / ja / en) bundle
/// byte-identical to what shipped before OI-0032. An explicit
/// `--target-direction ltr` on an Arabic run therefore renders LTR by absence
/// of the attribute.
pub fn dir_attr(mode: DirectionMode, language_label: &str) -> &'static str {
    match mode {
        DirectionMode::Rtl => RTL_ATTR,
        DirectionMode::Ltr => "",
        DirectionMode::Auto => {
            if label_is_rtl(language_label) {
                RTL_ATTR
            } else {
                ""
            }
        }
    }
}

/// Resolve the effective mode: an explicit flag wins, else the profile's
/// value, else `Auto`. The profile value is already validated at load time
/// (unknown values are warned about and normalized away); it is re-parsed
/// defensively here so a hand-built `ProfileMetadata` cannot smuggle in a
/// direction this crate does not understand.
pub fn resolve_mode(flag: Option<DirectionMode>, profile_value: Option<&str>) -> DirectionMode {
    if let Some(mode) = flag {
        return mode;
    }
    let value = profile_value.unwrap_or("").trim();
    if value.eq_ignore_ascii_case("rtl") {
        DirectionMode::Rtl
    } else if value.eq_ignore_ascii_case("ltr") {
        DirectionMode::Ltr
    } else {
        DirectionMode::Auto
    }
}

/// Does the label's primary subtag name a right-to-left language? The label is
/// split on the BCP-47 `-` separator (and `_`, which appears in POSIX-style
/// labels), and the first token is matched case-insensitively against the
/// table. Anything else — including expressive labels like
/// `"Korean (formal)"` — misses and resolves to LTR.
fn label_is_rtl(language_label: &str) -> bool {
    let primary = language_label
        .trim()
        .split(['-', '_'])
        .next()
        .unwrap_or("")
        .trim();
    if primary.is_empty() {
        return false;
    }
    let lowered = primary.to_ascii_lowercase();
    RTL_PRIMARY_SUBTAGS.contains(&lowered.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The `auto` table matches on the primary subtag, case- and
    /// separator-insensitively, and emits nothing for LTR labels.
    #[test]
    fn auto_rtl_table() {
        for label in [
            "ar", "ar-EG", "he", "iw", "fa_IR", "ckb", "AR", "  ur  ", "ps-AF",
        ] {
            assert_eq!(
                dir_attr(DirectionMode::Auto, label),
                " dir=\"rtl\"",
                "{label} should resolve RTL"
            );
        }
        for label in ["ko", "en-US", "ja", "zh-Hant", "ku", "", "   "] {
            assert_eq!(
                dir_attr(DirectionMode::Auto, label),
                "",
                "{label:?} should resolve LTR (no attribute)"
            );
        }
    }

    /// Documents the hint's best-effort limit: labels that are language
    /// *names* rather than tags miss the table and stay LTR. The explicit
    /// flag / profile key is the escape hatch (ADR-0013 amendment).
    #[test]
    fn expressive_labels_stay_ltr() {
        for label in [
            "Korean (formal)",
            "Arabic",
            "العربية",
            "Persian, literary",
            "Hebrew (modern)",
        ] {
            assert_eq!(
                dir_attr(DirectionMode::Auto, label),
                "",
                "{label:?} is not tag-shaped and must stay LTR"
            );
        }
    }

    /// An explicit mode ignores the table in both directions.
    #[test]
    fn explicit_mode_wins() {
        assert_eq!(dir_attr(DirectionMode::Rtl, "ko"), " dir=\"rtl\"");
        assert_eq!(dir_attr(DirectionMode::Rtl, ""), " dir=\"rtl\"");
        assert_eq!(dir_attr(DirectionMode::Ltr, "ar"), "");
        assert_eq!(dir_attr(DirectionMode::Ltr, "he-IL"), "");
    }

    /// Flag > profile > built-in auto, with unknown profile values (which the
    /// loader already normalizes away) falling back to auto.
    #[test]
    fn resolve_mode_precedence() {
        assert_eq!(
            resolve_mode(Some(DirectionMode::Ltr), Some("rtl")),
            DirectionMode::Ltr,
            "flag beats profile"
        );
        assert_eq!(
            resolve_mode(Some(DirectionMode::Auto), Some("rtl")),
            DirectionMode::Auto,
            "an explicit --target-direction auto also beats the profile"
        );
        assert_eq!(
            resolve_mode(None, Some("rtl")),
            DirectionMode::Rtl,
            "profile beats the default"
        );
        assert_eq!(resolve_mode(None, Some("ltr")), DirectionMode::Ltr);
        assert_eq!(resolve_mode(None, Some("auto")), DirectionMode::Auto);
        assert_eq!(
            resolve_mode(None, Some("sideways")),
            DirectionMode::Auto,
            "unknown profile value falls back to auto"
        );
        assert_eq!(resolve_mode(None, None), DirectionMode::Auto);
    }
}
