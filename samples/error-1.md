This is a deeper failure mode than the original R0004-0001 case. Let me walk through what's actually happening.

## Pattern across your four reproducers

All four divergences sit at a **list-item → paragraph** boundary:

| Failure | Position | Flanking source blocks |
|---|---|---|
| paragraph → code-block | block 5 | `li-0007` ↔ `p-0008` |
| paragraph → code-block | block 2 | `li-0003` ↔ `p-0004` |
| count 15 vs 16 | — | `li-0027` ↔ `p-0028` |
| count 3 vs 4 | — | `p-0004` ↔ `p-0005` |

Three of four are the same boundary class. That's not random sampling noise — it's a stable LLM-output pattern for this source structure.

## Why per-block fallback isn't recovering it

`FallbackPerBlock` does this when the first reparse rejects:
1. Identify the *regenerated* block at the divergence position.
2. Map that position → the *source* block that produced it.
3. Mark that source block as `FallbackSource` (reuse source bytes for it).
4. Re-regen, re-reparse. If still bad → `Err`.

In your case, transync identifies `p-0008` (the paragraph at regenerated position 5) and falls IT back. But the regenerated code-block at position 5 isn't an artifact of `p-0008`'s translation — it's an artifact of **`li-0007`'s** translation bleeding across the boundary. The list item's translated output ends with something Comrak then reads as opening a code block (most likely 4-space-indented continuation that's a Comrak indented-code-block trap, or a stray closing ` ``` ` the LLM included to "balance" the upcoming fenced block).

When transync re-regenerates with only `p-0008` falling back to source bytes, the `li-0007` translation is still present, still emits the boundary-poisoning content, and the reparse still produces a code-block at position 5.

That's why retries don't help either: the LLM consistently translates the list item the same way, so the same boundary contamination repeats. Cache-bypass on retry just gets fresh translations of the *same blocks*, and the LLM's bias is stable.

## Confirmation from your MCP log

```
WARN transync::pipeline: full reparse rejected output (...); marking 1 block(s) as fallback
```

`marking 1 block(s)` confirms transync only chose ONE source block to fall back. Empirically that one is `p-0008`. The actual culprit is `li-0007`. Transync's blame-attribution is too narrow.

## Three paths forward

**1. Operator workaround — `FallbackAll` for this profile.** Set `TranslateOptions.full_reparse_failure = FallbackAll` so when per-block fails, transync returns source bytes for the entire document instead of `Err`. The UI shows the source unchanged on the right pane, so at least the row isn't dead. We'd surface this via a new `PromptProfile` field or a global MCP config knob.

**2. Stricter system prompt.** The current `claude-code-default.toml` system prompt ("Translate prose into Korean. Do not translate identifiers / file paths / commands. Output only the translation; transync handles structural Markdown preservation.") doesn't tell the LLM to preserve LIST STRUCTURE specifically. Adding a rule like "Never indent continuation lines. Never emit ` ``` ` outside the source's existing fences. Each list item must be a single line; do not insert blank lines inside or after a list item's translation." may reduce the boundary-poisoning rate. Won't be 100%, but it changes the LLM's prior.

**3. Upstream fix in transync.** Per-block fallback should widen its radius when the first attempt fails — fall back the divergent block AND its neighbors (or its preceding source block specifically, since list→paragraph is the dominant failure direction). Alternatively, transync could escalate `FallbackPerBlock → FallbackAll` automatically on second-reparse failure rather than returning `Err`.

(3) is the right structural fix. (1) is an immediate consumer-side mitigation that doesn't depend on transync changes. (2) is cheap and might cut the rate by half.

I'd recommend filing **upstream finding R0006** to transync covering the radius-widening and the auto-escalation. Want me to write that report? It'd cite your four reproducers + the specific MCP log line proving only one block was falling back, and suggest two alternative fix shapes (widen radius vs. auto-escalate on second failure).
