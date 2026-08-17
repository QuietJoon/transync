## Validation retry — column count

The table below is the unit the `MockTranslator(rejects-then-accepts)` fixture targets. Attempt 1 returns a 2-column result (rejected by `per_kind`); attempt 2 returns a 3-column result and is accepted.

| Field   | Type   | Notes                  |
|---------|--------|------------------------|
| id      | u64    | unique within document |
| name    | string | display name           |
| status  | enum   | open / closed / merged |
| author  | string | committer email        |
