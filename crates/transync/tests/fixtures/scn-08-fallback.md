## Fallback to source

The third paragraph in this fixture (`p-0003`) is the unit `MockTranslator::always_fails_unit("p-0003")` rejects on every attempt. After `max_per_unit_validation_retries` exhausts, the alignment map's row for `p-0003` will carry `fallback_status: fallback_source` and the rendered target HTML for that block will carry `data-fallback="fallback_source"`.

The remainder of the document translates normally.

This is the third paragraph — the one that always fails. It must remain present in the rendered target as a sync anchor, in source language, so the JS engine can still align it.

The fourth paragraph closes the fixture. It is unaffected.
