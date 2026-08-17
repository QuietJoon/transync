Already done — committed as `a892d49`.

- `cargo test --workspace` → **199 passed, 0 failed** (one transient flake on `scn_19_install_hook_project` that passed on a clean re-run; cwd-sensitive test).
- Commit message covers both `TransyncError::stable_code()` and the inherited `FullReparseFailure::FallbackPerBlock` default.

The wakeup prompts have been racing with completed work — bqdql3at4 ended at exit 0, b5i8nztcx caught the flaky test, bysp23vgv re-ran clean at 199/0, then I committed. No further action needed on this thread.
