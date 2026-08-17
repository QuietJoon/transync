//! Parse-side error type. `transync-core`'s `TransyncError` wraps it via
//! `#[from]` — the cross-crate From derive works unchanged.

/// Errors from [`crate::parser::parse`].
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
    /// [`parser::MAX_BLOCK_NESTING_DEPTH`](crate::parser::MAX_BLOCK_NESTING_DEPTH),
    /// so [`crate::parser::parse`] refused it without parsing.
    ///
    /// `depth` is the pre-scan's *upper bound* on the source's nesting, not a
    /// measured tree depth — the document was never built. A stack overflow
    /// is an abort rather than a catchable panic, so the only safe answer to
    /// input this deep is to decline it; see `parser::depth` for the bound
    /// and the measurements behind its value.
    #[error(
        "block nesting too deep: a pre-scan bounds this source at {depth} nested block \
         containers, over the {limit}-level maximum"
    )]
    TooDeeplyNested { depth: usize, limit: usize },
}
