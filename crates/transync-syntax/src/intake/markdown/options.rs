//! The one canonical Comrak configuration.
//!
//! Every parse in the workspace — [`super::parse`], regen's splice reparse,
//! the renderer's per-pane parse, core's fragment/full-reparse validators and
//! its structural fingerprint probes — goes through the same flags, so the
//! AST shape they each see is the shape block IDs were assigned against
//! (ADR-0004). Splitting the config out of the walker keeps that guarantee
//! visible in one file instead of at the top of the traversal.
//!
//! TRACE: ADR-0004

/// Build the GFM-flavored Comrak options used everywhere in the pipeline.
///
/// TRACE: ADR-0004
pub(super) fn gfm_options<'c>() -> comrak::ComrakOptions<'c> {
    let mut opts: comrak::ComrakOptions<'c> = comrak::ComrakOptions::default();
    opts.extension.table = true;
    opts.extension.tasklist = true;
    opts.extension.strikethrough = true;
    opts.extension.autolink = true;
    opts.parse.smart = false;
    opts.render.unsafe_ = false;
    opts
}

/// Public accessor for the canonical GFM options. Other modules (regen,
/// render) reparse with the same flags so the AST shape matches.
///
/// Re-exported as `markdown::comrak_options` — that path is what the rest of
/// the workspace calls, and it is load-bearing.
///
/// TRACE: ADR-0004
pub fn comrak_options() -> comrak::ComrakOptions<'static> {
    gfm_options()
}

/// The canonical GFM options with comrak's `broken_link_callback` set: a
/// reference whose label the parsed text does not define itself is handed to
/// `callback`, and the destination it returns becomes the link.
///
/// The same flags as [`comrak_options`], so a parse that resolves through a
/// callback sees exactly the AST shape every other parse sees. Used by
/// `transync-core` to resolve a block's references against the definitions of
/// the whole document (R0013-0003); nothing in this crate calls it, so the
/// parse path [`super::parse`] and the renderers take is unchanged.
///
/// TRACE: ADR-0004
pub fn comrak_options_with_broken_link_callback<'c>(
    callback: std::sync::Arc<std::sync::Mutex<comrak::BrokenLinkCallback<'c>>>,
) -> comrak::ComrakOptions<'c> {
    let mut opts = gfm_options();
    opts.parse.broken_link_callback = Some(callback);
    opts
}
