//! Parse-side error type. `transync-core`'s `TransyncError` wraps it via
//! `#[from]` — the cross-crate From derive works unchanged.

/// Errors from [`crate::intake::markdown::parse`] and from
/// [`crate::intake::html::check_nesting_depth`], the HTML intake's guard.
///
/// Non-exhaustive: new variants may be added in minor releases; match with
/// a wildcard arm.
///
/// TRACE: SCN-01
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ParseError {
    #[error("comrak parse failed: {0}")]
    Comrak(String),

    #[error("unsupported syntax: {0}")]
    Unsupported(String),

    /// The source nests block containers deeper than
    /// [`markdown::MAX_BLOCK_NESTING_DEPTH`](crate::intake::markdown::MAX_BLOCK_NESTING_DEPTH),
    /// so [`crate::intake::markdown::parse`] refused it without parsing.
    ///
    /// `depth` is the pre-scan's *upper bound* on the source's nesting, not a
    /// measured tree depth — the document was never built. A stack overflow
    /// is an abort rather than a catchable panic, so the only safe answer to
    /// input this deep is to decline it; see `markdown::depth` for the bound
    /// and the measurements behind its value.
    #[error(
        "block nesting too deep: a pre-scan bounds this source at {depth} nested block \
         containers, over the {limit}-level maximum"
    )]
    TooDeeplyNested { depth: usize, limit: usize },

    /// The HTML document opens an element deeper than
    /// [`transync_html::MAX_NESTING_DEPTH`], so
    /// [`crate::intake::html::check_nesting_depth`] refused it before the
    /// intake built anything (R0013-0002, DCR-0063).
    ///
    /// `offset` is the byte offset, in the source as given, of the start tag
    /// that would have opened one element more than `limit`. Unlike
    /// [`Self::TooDeeplyNested`] it is measured, not bounded: the scanner
    /// walks the document's own stack and stops at the first element past the
    /// ceiling. Past it the cost is memory and time rather than stack (the
    /// intake's per-block paths and the scanner's scope searches both grow
    /// with depth).
    ///
    /// The depth is the scanner's stack, which on malformed markup is not a
    /// browser's: a run of unclosed `<a>`, `<nobr>`, `<button>` or headings
    /// counts one level per tag where a browser closes the previous one, so
    /// this can refuse a document a browser builds a few levels deep.
    /// Closing those tags clears it. [`transync_html::MAX_NESTING_DEPTH`]
    /// lists the rules the scanner does not model.
    #[error(
        "HTML nesting too deep: the element opening at byte {offset} nests past the \
         {limit}-element maximum"
    )]
    HtmlTooDeeplyNested { offset: usize, limit: usize },
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The HTML refusal says what it measured, in HTML's terms: an element at
    /// a byte offset, against the element ceiling. The Markdown variant's
    /// "pre-scan bound" on "block containers" would be the wrong sentence.
    #[test]
    fn the_html_refusal_names_its_offset_and_its_ceiling() {
        let msg = ParseError::HtmlTooDeeplyNested {
            offset: 2560,
            limit: 512,
        }
        .to_string();
        assert_eq!(
            msg,
            "HTML nesting too deep: the element opening at byte 2560 nests past the \
             512-element maximum"
        );
    }
}
