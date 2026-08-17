## Reference table

This is the **skeleton-sized** SCN-03 fixture (12 data rows). SL-03 will
expand it to 200 rows via `tests/common/fixture_gen.rs::generate_scn_03`.
The 12-row version still exercises the row-window code path when the
`max_units_per_batch` knob is set to a small value during the test.

| Code   | Name              | Description                                         | Notes                  |
|--------|-------------------|-----------------------------------------------------|------------------------|
| R-0001 | block-id          | Stable kind-prefixed sequential identifier          | see ADR-0005           |
| R-0002 | source-hash       | SipHash-1-3 over canonical block bytes              | u64 integrity field    |
| R-0003 | translator        | Async trait crossing the LLM boundary               | see contracts.md §1    |
| R-0004 | profile           | TOML system prompt + glossary + batching hints      | see contracts.md §2    |
| R-0005 | alignment-map     | Durable JSON wire, schema_version 1.0.0             | see contracts.md §3    |
| R-0006 | data-sync-id      | HTML attribute carrying the BlockId on each anchor  | see contracts.md §4    |
| R-0007 | retry-policy      | max-per-unit / max-oversize / max-per-batch         | see contracts.md §5    |
| R-0008 | cli-contract      | --input / --output / --map / --html-out             | see contracts.md §6    |
| R-0009 | from_env          | Resolves OPENAI_API_KEY + model + base_url          | see contracts.md §7    |
| R-0010 | renderer          | Two pane fragments + templated demo shell           | see ADR-0006           |
| R-0011 | comrak            | GFM parser; AST-oriented                            | see ADR-0004           |
| R-0012 | workspace         | Three crates: transync, transync-openai, cli        | see ADR-0003           |
