## Safe fence regeneration

The translator must return code-block content only — never the surrounding fence. Rust regenerates the fence with enough backticks to safely contain the body, and preserves the original info string.

```rust
/// Demonstrate a code block whose body contains backtick runs.
///
/// The example below shows ```triple backticks``` inline; the
/// regenerator must pick a fence at least one backtick longer
/// than the longest run in the body.
pub fn demo() -> Result<&'static str, &'static str> {
    // Returns the first error encountered.
    let payload = "```nested```";
    if payload.is_empty() {
        return Err("empty payload");
    }
    Ok(payload)
}
```

When the body contains a four-backtick run, the regenerator picks a five-backtick fence, and so on. Translated comments are accepted by validation; identifiers are preserved unless explicitly altered by the model.
