# `benchmark/`

Measurement harnesses that inform a decision and then stay put as its evidence.

**Nothing here is a workspace member**, and the harnesses are not all the same
shape. A Rust one carries its own `[workspace]` table, which detaches it from
the root workspace, so `cargo build`/`cargo test` at the repo root never builds
it, the publication roster is untouched, and a heavy benchmark dependency (a
language model, a corpus, a criterion harness) cannot reach the shipped crates.
`crates/transync/tests/workspace_publication.rs` scans `crates/` only, so a
manifest here does not disturb it. `scroll-frame` carries no manifest at all —
it is a Node profiler script (`profile.mjs`) beside its results.

Run one from inside its own directory:

```bash
cd benchmark/lang-detect && cargo run --release   # Rust harness
node benchmark/scroll-frame/profile.mjs           # Node harness
```

| directory | question it answered | decided |
|---|---|---|
| `lang-detect` | which language detector should back the "is this already the target language?" gate (ticket `eb1d89`) | 2026-09-02 |
| `scroll-frame` | does active-block selection overrun the 60 Hz frame budget on a large document (ticket `dd21ad`, OI-0016) | 2026-09-02 |

A benchmark earns its place here when its result is cited by a record. If a
result stops being cited, delete the directory rather than leaving a harness
nobody runs — the git history keeps it.
