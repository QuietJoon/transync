// Fixture for `src/containment.rs`'s walker test (R0012-0021). Never compiled.
// A child module one directory down that names the backend: the walker must
// reach it, and the checker must flag it.

use whichlang::Lang;

pub(crate) fn label(_: Lang) {}
