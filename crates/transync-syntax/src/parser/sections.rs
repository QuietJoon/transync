//! THE heading-scope stack.
//!
//! One rule, one implementation: a heading closes every open scope at or
//! above its own level, then opens its own. The surviving chain is what
//! [`super::Block::section_path`] records for every block emitted under it.
//!
//! The rule used to exist twice — here, and again in the eagerly-built
//! `Document::hierarchy` pass that re-derived it from the flat block list.
//! That pass was deleted (it had no consumer); this stack is the only place
//! the scope rule lives, and [`super::Section`] is the shape a future
//! hierarchical-alignment revision (OI-0005) would project *from* it rather
//! than re-derive beside it.
//!
//! TRACE: SCN-09

use crate::id::BlockId;

/// Open heading scopes, outermost first, each with the level that opened it.
#[derive(Debug, Default)]
pub(crate) struct SectionStack {
    open: Vec<(BlockId, u8)>,
}

impl SectionStack {
    /// The heading chain enclosing whatever is emitted next.
    pub(crate) fn current_path(&self) -> Vec<BlockId> {
        self.open.iter().map(|(id, _)| id.clone()).collect()
    }

    /// Close every scope at or below `level`, then report the chain that
    /// survives — the section path the heading itself belongs to. Call this
    /// *before* emitting the heading; the heading's own scope opens only once
    /// it has an ID ([`SectionStack::open_scope`]).
    pub(crate) fn close_through(&mut self, level: u8) -> Vec<BlockId> {
        while let Some((_, lvl)) = self.open.last() {
            if *lvl >= level {
                self.open.pop();
            } else {
                break;
            }
        }
        self.current_path()
    }

    /// Open the emitted heading's own scope.
    pub(crate) fn open_scope(&mut self, heading_id: BlockId, level: u8) {
        self.open.push((heading_id, level));
    }
}
