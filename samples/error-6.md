The MCP binary you're running predates my retry fix (commit `eaacb20`). Look at what's missing from the log:

For event `3ca0cfcb` we should see TWO `broadcast complete frame_kind="event_translated"` lines — one for the transient `Pending { reason: "retry_in_progress" }` I added before the actual retry runs, then one for the final `Failed`. The log only shows the second.

That means the running binary is on the pre-fix code path: `handle_retry_request` jumps straight into `resolve_translate_broadcast` with no transient Pending broadcast and no cache reset.

**Action: rebuild + restart MCP.** Whatever process is running came from a build before `eaacb20` landed.

```sh
# in /Volumes/Common/QJoon/resp-translator
cargo build -p copy-transfer-mcp --release   # or --bin copy-transfer-mcp without --release if you've been running debug
# kill the running MCP, then start the new binary
```

After restart, clicking Retry should produce:
1. An immediate transient `Pending` broadcast (visible "translating…" in the UI), and
2. A fresh-cache run through transync — different translations may regenerate cleanly.

Caveat I already flagged: if transync's regen failure is deterministic for this Korean-translation-of-an-ordered-list pattern, even a fresh-cache attempt may produce another regen-count mismatch. The transient Pending will at least *show you the attempt happening*, but the underlying transync regen-robustness issue (R0004-0001) is still upstream.

If after restart the retry log is still missing the first `broadcast complete` line, that's a different bug and I'll dig further — but I'd bet on stale binary first.
