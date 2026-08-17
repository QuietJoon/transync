Committed `a892d49`. Two changes:

1. **MCP uses `TransyncError::stable_code()`** — replaces my hand-rolled mapping that collapsed everything except provider errors to `"transync_other"`. Now consumers see the real diagnostic surface: `parse_failed`, `provider_error`, `validation_failed`, `regen_failed`, `profile_failed`, `alignment_failed`, `internal`.
2. **Listener's `static_code` map extended** to recognize all seven new codes plus the existing pre-transync set + retry/watchdog synthetics. UI can branch on them; nothing collapses to `"other"` silently anymore.

**On the regen mismatch**: nothing else needs changing on my side. transync's `FullReparseFailure::FallbackPerBlock` is the new `Default` for `TranslateOptions`, and my `to_transync_options` builds with `..TranslateOptions::default()` — so we automatically inherit the safer policy. The class of failure that produced your `[transync_other] regenerated block count N differs from source top-level count M` errors now auto-recovers per-block: divergent blocks get `FallbackStatus::FallbackSource` (source bytes), regen + reparse retry once, and only return `Err` if the second reparse still fails. The retry button you reported as "doing nothing" will, on most documents, succeed automatically without ever reaching the failure path.

199 tests still pass; clippy + fmt clean.
