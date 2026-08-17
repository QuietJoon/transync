Report filed at `/Volumes/Common/QJoon/transync/reviews/0004.md` (claim token `223CAC76-…-6D5B0C55B685`). Format mirrors transync's existing review-slot pattern so their gate workflow can pick it up directly.

**One high-severity finding**: full-reparse block-count mismatch returns `Err` with no per-block fallback path, even though transync's per-unit fallback machinery already exists for in-pipeline rejections. Three recommendations, in order of consumer value:

1. **Per-block fallback for post-regen reparse failure** — locate the divergent block(s), fall those back to source bytes, re-regen.
2. **Diagnostic detail** — include source-side `BlockId`s flanking the mismatch in the error string (e.g., "regen lost a block between `p-0007` and `h2-0008`").
3. **Caller-controlled severity** — add `TranslateOptions.full_reparse_failure: { Hard | FallbackPerBlock | FallbackAll }` so consumers pick the policy.

**Two low-severity observations folded in:**

- `normalize_for_top_level` (`validate/full_reparse.rs:94-105`) may not cover loose-list semantics or code-fence info-string drift; suggest fixture tests.
- `TransyncError` variants would benefit from a `stable_code()` method so multiple consumers map them consistently.

Citations point at exact files/line ranges. The "what we did downstream as a workaround" context (resp-translator's retry uses a throwaway cache, broadcasts transient Pending) is included so the maintainer understands what consumers have already tried.
