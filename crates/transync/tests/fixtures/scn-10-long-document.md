## Section 1

This is the **skeleton-sized** SCN-10 fixture (6 sections). SL-10 will expand it to 30 sections via `tests/common/fixture_gen.rs::generate_scn_10` so it crosses the default `target_output_tokens=8000` threshold and forces the batcher to yield ≥ 4 batches.

The skeleton-sized fixture still exercises the multi-batch code path when the test sets `max_units_per_batch=2`.

## Section 2

Three short paragraphs.

A second short paragraph.

A third short paragraph.

## Section 3

```rust
fn embedded_code_block() -> &'static str {
    "in section 3"
}
```

The code block above checks that the batcher does not split fenced blocks across batches.

## Section 4

| col-a | col-b | col-c |
|-------|-------|-------|
| r1a   | r1b   | r1c   |
| r2a   | r2b   | r2c   |

The table above checks that whole-block table units stay in one batch.

## Section 5

Plain prose section. Nothing structural.

A second paragraph in section 5.

## Section 6

Closing section with one paragraph and one bullet list.

- item one
- item two
- item three
