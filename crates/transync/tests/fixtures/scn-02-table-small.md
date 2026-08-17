## Configuration knobs

The table below summarizes the public knobs exposed by `TranslateOptions`. Defaults reflect the values locked in `contracts.md` §5.

| Name                               | Type    | Default              | Notes                                                |
|:-----------------------------------|:-------:|---------------------:|------------------------------------------------------|
| source_language                    | string  |                 auto | BCP-47 code or the literal `auto`                    |
| target_language                    | string  |                    – | required                                             |
| model_id                           | string  |   gpt-5-chat-latest  | threaded into the cache key                          |
| max_per_unit_validation_retries    | u32     |                    2 | total attempts = retries + 1                         |
| max_per_batch_provider_retries     | u32     |                    1 | bounded transport retries                            |
| profile                            | option  |                 None | falls back to the embedded default profile           |

These defaults serve nine of the fourteen MVP scenarios out of the box; only SCN-03, SCN-08, SCN-10, SCN-11, and SCN-12 need overrides.
