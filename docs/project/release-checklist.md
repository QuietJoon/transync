# Release Checklist

The ritual for cutting a tagged `transync` release, in the order it has to
happen. This is a **living document**: edit it in place as the ritual
changes, and add a step the moment a release misses one.

## Authority

Under `BL-2026-07-B` (`docs/project/design-baseline-2026-07.md`) this file is
*procedure*, not contract. It sits below every level of that baseline's
authority hierarchy and only says how to ship what those documents already
govern; where it disagrees with one of them, the higher document wins and
this file is what gets fixed.

- **Whether a change may ship, and whether it is breaking**, is decided by
  `docs/architecture/contracts.md` — §0 (public Rust surface), §1 (stability
  rules), §3 (alignment-map `schema_version`). Never decided here.
- **Why a gate exists** lives in its own record: the live-endpoint gate is
  DCR-0015 Part C (OI-0030), the standing wasm gate is DCR-0017 as widened by
  DCR-0020, the rustdoc gate is DCR-0018. This file says *when to run* a
  gate; removing or weakening one is a design change with its own record.
- **What a given release actually did** is recorded in `CHANGELOG.md`,
  `docs/project/status.md`, and `docs/project/phase-state.yaml`. This file is
  the instruction; those are the evidence.

Editing this checklist needs no Design Change Record. Changing a gate it
names does.

## When it applies

Every annotated `vX.Y.Z` tag. Steps 10–12 (the live-endpoint gate) are
conditional on what the release range touches; everything else is
unconditional.

### v0.4.0 shipped untagged, then was tagged — owner decisions, 2026-08-13 and 2026-08-20

**Superseded. `v0.4.0` is tagged.** The decision below stood from 2026-08-13
until the release had already shipped, and the owner reversed it the same day
the release landed: an **annotated `v0.4.0` tag** now points at the release-prep
commit from step 24, and it is pushed to the `backup` mirror.

What reversed it was not a change of mind about this repository's history —
that reasoning, kept below, still holds — but a use the original decision did
not weigh: **consumers need a ref to depend on.** A tag serves that whether or
not the commit's ancestry is the development that produced the release. The tag
makes no claim about ancestry; it names which tree is v0.4.0.

So **steps 1, 15, 20, 25, 26, 27 and 28 run as written** — their v0.4.0
carve-outs are spent, and each says so where it stands. Two carve-outs are
**not** spent, because they were never about the tag: **steps 3 and 10** rest on
the history restart, and both facts are still true — no `v0.3.0` tag exists to
name, and this history is one root commit below the release, so a
`git log v<prev>..HEAD` or `git diff v<prev>..HEAD` range still cannot be
formed. Do not read "the tag exists now" as retiring those.

The original decision and its reasoning, kept because the next reader needs to
know why the release shipped the way it did:

**Scope: v0.4.0 only.** That release is recorded in `CHANGELOG.md` and in the
section F records, and **no `v0.4.0` tag is created for it**. Tagging resumes
at the next release, which runs every step below exactly as written.

The reason is a fact about this object store, not a preference. This
repository's git history has been restarted **twice** — on 2026-08-10
(`docs/project/git-history-loss-2026-08-10.md`) and again on 2026-08-17
(`docs/project/git-history-loss-2026-08-17.md`) — each time after the object
store lost objects to a file-sync client writing inside `.git`, 30 the first
time and 33 the second. Each restart replaced the whole graph, so **this
history has exactly one commit below the work in progress**: the restart
commit, whose message begins "chore: the repository restarts from a verified
working tree". Name it by resolving it rather than by quoting a hash — an
earlier revision of this file quoted the 2026-08-10 restart's hash, and the
2026-08-17 restart made that instruction unfollowable:

```bash
git rev-list --max-parents=0 HEAD    # the root commit of whatever history this clone holds
```

`git tag -l` returns **empty**: the three previously shipped tags did not
survive, and the commits they pointed at are not in this clone. Tagging v0.4.0
here would put a release tag on a commit that does not carry the release's
history.

Nothing below is deleted for this — every step that needs a tag needs one
again at the next release. Where a step cannot be followed without a tag
*now*, it says what to do for v0.4.0 instead: steps 1, 3, 10, 15, 20, 25, 26,
27 and 28.

*(Since the 2026-08-20 reversal that list has two live members, not nine.
Steps 1, 15, 20, 25, 26, 27 and 28 needed a tag and now have one, so each
carries a spent-note where it stands and otherwise reads as written. **Steps 3
and 10 are still live**, because they never depended on the tag: no `v0.3.0`
exists to name, and this history is one root commit below the release, so a
`v<prev>..HEAD` range cannot be formed either way.)*

**The step numbers are cited from outside this file** — the failure
messages in `crates/transync/tests/workspace_publication.rs` (steps 17 and
19), `docs/implementation/module-map.md` (step 6),
`docs/project/open-issues.md` and ADR-0019 (step 18), and past `CHANGELOG`
entries (steps 17 and 18). ADR-0019 and a shipped changelog entry are
snapshot records and are never edited in place, so renumbering this list
would falsify a frozen document. Add a step at a number that leaves the
existing ones where they are — step 0 below is the first one added that
way, and step **2a** the second, taking a letter because no integer was
free where it belonged.

---

## A. Preflight

0. **Verify the object store before trusting anything in it.**

   ```bash
   git fsck --no-progress --connectivity-only
   ```

   Pass is: no `missing <type> <sha>` line and no `broken link from <sha>
   to <sha>` line. `dangling` lines are normal — abandoned tips and
   amended commits leave them, and this clone carries a handful at any
   time.

   This step exists because the store has already lost an object. On
   2026-08-07 the `docs/` subtree of commit `494bc9c` disappeared from
   `.git/objects` minutes after the commit was written, and every `git
   show` or `git diff` crossing that commit failed with `fatal: unable to
   read tree` (ticket `7a7feb`; `docs/Troubleshooting.md` carries the
   diagnosis and the repair), and twice more since — 30 objects on
   2026-08-10 and 33 on 2026-08-17, each costing the whole history. The
   cause — a file-sync client re-materializing files under `.git/` while git
   writes them, on the external volume this worktree lives on — was
   **confirmed with physical evidence on 2026-08-17, and excluded on paper
   the same day** by adding `.git` to the client's ignore list — written, not
   yet proven, since nobody has confirmed the client re-read that list
   (`docs/project/git-history-loss-2026-08-17.md`). It had been parked since
   2026-08-07, which is how the second and third events happened. This check
   stays anyway, and not as a formality: `git fsck` is what established the
   damage in all three, and a removed cause is a claim that has to keep being
   true. A release is the worst place to find out: the released
   commit would name a history nobody can clone, and for v0.4.0 that commit
   is what the `v0.4.0` tag points at — a tag on an unreadable commit is worse
   than no tag, which is why this check precedes the tagging step rather than
   following it.
   Same-day detection is also what keeps the damage cheap, because the lost
   tree was reconstructible only while the surrounding commits still held
   every blob it referenced, and a lost *blob* has no equivalent trick.

   `--connectivity-only` is the form that catches this failure — an object
   that is absent rather than damaged. `git fsck --full` additionally
   reads object content; at this repository's size both finish in a
   fraction of a second, so run `--full` whenever there is any doubt.

   Related, and already set on this worktree: `gc.auto 0` (`git config
   --local gc.auto 0`), so no automatic gc can start inside one session's
   commit while a second session writes into the same tree. It forbids
   nothing an operator does deliberately — an explicit `git gc` at a quiet
   point is exactly the replacement it assumes.

1. **Fix the commit you are going to tag.** `git status` clean, on `master`
   (this project commits directly to `master`), and everything the release
   contains already committed — a tag points at a commit, never at a working
   tree. Every gate below runs against *that* commit.

   v0.4.0's carve-out here is spent: it fixed one commit — the release-prep
   commit from step 24 — and since 2026-08-20 the `v0.4.0` tag points at that
   same commit, so the step reads as written with nothing removed.

2. **Pick the version number under the stability rules, not by feel.**
   `contracts.md` §0/§1 decide what counts as breaking; a break needs a
   sanctioned window. The 0.3.0 and 0.4.0 windows are both **used and
   closed**; the **v0.5.0 window is open** (`ff788f7`, 2026-08-24 — recorded
   in `phase-state.yaml` and in `status.md`'s workspace-version bullet), and
   it is closed by the v0.5.0 release itself. Additions protected by
   `#[non_exhaustive]`, new defaulted trait methods, and new modules are
   additive and need no window.

**2a. Confirm the version's window-closing condition is met — and know which
condition it is.** A release number is not only a semver claim. Some numbers in
this project carry an **additional, owner-stated condition** that has to be
discharged before the release may be cut at all, and step 2's stability rules say
nothing about it. This step is where that condition is named and checked; it is
deliberately written to generalise, so each version records its own condition here
as it is stated, and a version with none says so.

- **v0.5.0 — stated by the owner 2026-08-24.** *`transync` bumps to 0.5.0 proper
  only after every **registered task** and every **open issue** is resolved,
  excluding those explicitly deferred.* "Registered" is the **union** of the
  TicGit queue (`ti list --all`) and the open-issue register
  (`docs/project/open-issues.md`) — an item closed in one and open in the other
  still counts. "Explicitly deferred" means a **recorded** deferral naming a
  falsifiable trigger, not merely blocked and not merely unqueued; such an item
  **re-counts the moment its trigger fires**. The open v0.5.0 breaking window is
  not this condition — the window says what *may* ride the release, this says
  whether the release may happen.
- **Where the answer lives:** `docs/backlog.md`. Its header states this rule and
  carries the census — every entry classified as counting, deferred-with-a-
  recorded-condition, or resolved/historical — and `status.md` names that file as
  the index of open items. **Re-derive the counts against `ti list --all` and
  `open-issues.md` instead of reading the last sweep's numbers**; a stale census
  answers this step wrongly and silently, which is exactly how the failure below
  happened.
- **v0.1.0 through v0.4.0 carried no such condition**, which is why this step did
  not exist for them: they were gated by step 2's stability rules and the standing
  gates in section B, and by nothing else.

Why this is a step and not a convention: the 0.4.0 window was declared
used-and-closed while six wave-0/1 finding tickets sat unindexed — one of them
`ticgit:e77173bb`, a *live* `contracts.md` §4a break — and a sweep on 2026-08-24
(`e89b537`) then found **18 of the 24** open tickets missing from the file
`status.md` calls "the index of open items". The condition was answerable the
whole time. Nothing in the ritual asked.

**On the number `2a`.** *When it applies* forbids renumbering, because these step
numbers are cited from outside this file, and there is no free integer between 2
and 5. So this step takes a letter — the same rule step 0 followed, applied where
no integer was available. A future insertion should do the same rather than shift
anything.

3. **Audit the CHANGELOG for completeness against the commit range**, not
   from memory: read `git log --oneline v<prev>..HEAD` alongside the
   `[Unreleased]` section and account for every commit. Entries are written
   per wave and some waves never write one — the v0.2.0 prep restored 19
   entries this audit found missing.

   **For v0.4.0 there is no `v0.3.0` to name, and after the 2026-08-17
   restart there is no range to walk either.** This clone's history is the
   root commit and nothing else, so `git log --oneline <root>..HEAD` spans
   **zero** commits — an empty audit that would read as a clean one. Do not
   run it and do not read its silence as completeness. **Audit the
   `[Unreleased]` entries against the tree instead**: take each entry and
   confirm the code, tests and records it claims are present; then sweep the
   surfaces step 4 names, plus `CHANGELOG.md`'s own `[Unreleased]` heading
   set, for anything the tree has that no entry mentions. The TicGit board —
   `ti list --all`, whose resolution comments name what each ticket closed —
   is the closest thing to a commit log this repository still has, and it is
   the second source to read the entries against. From the next release on,
   the range is
   `git log --oneline <this release's prep commit>..HEAD` again.

4. **Check the identity set for silent movement.** The alignment-map
   `schema_version` (contracts.md §3), the validation-report
   `schema_version` (contracts.md §3a — an independent axis, not the one
   below), `VALIDATION_SCHEMA_VERSION`, `CacheKey`, and the two
   byte-mirrored `sync.js` copies (`web/js/sync.js` and
   `crates/transync-cli/web/sync.js`). If any of them moved in this
   range it is a CHANGELOG entry, not a discovery made afterwards.

## B. Standing gates

Run all of them on the exact commit from step 1, and keep the output.

5. **`./scripts/smoke.sh`** — the aggregate gate. It runs `cargo build
   --workspace`, the standing wasm gate (`cargo check -p transync-syntax -p
   transync-wasm --target wasm32-unknown-unknown`), `cargo test --workspace
   -- --test-threads=4`, the CLI stub suite, the rustdoc gate
   (`RUSTDOCFLAGS="-D warnings" cargo doc --no-deps` over `transync-syntax`,
   `transync-core`, `transync`, `transync-openai`, and `transync-wasm` —
   every member with a library target; bin-only `transync-cli` is out),
   `scripts/build-wasm.sh` with its size budget, and a CLI dry run
   whose eight output files must all be non-empty. Host prerequisites — the
   `wasm32-unknown-unknown` rustup target, `wasm-pack`, and binaryen ≥ 121 —
   fail loudly rather than skipping, by design.

6. **`./scripts/test-browser.sh`** — the headless Playwright suite: SCN-13
   dual-pane sync plus the Track C wasm demo, including the CLI-vs-browser
   render parity pin. It regenerates its own CLI fixture and runs
   `build-wasm.sh`, so it inherits smoke's prerequisites plus Chromium.

7. **`cargo fmt --all -- --check`** and **`cargo clippy --all-targets
   --all-features -- -D warnings`.** The tracked pre-commit hook runs both
   plus the wasm gate and the rustdoc gate on the release-prep commit, so
   running them here is an early check, not a substitute — and the hook is
   never bypassed with `--no-verify`. Confirm the hook is live:
   `git config core.hooksPath` must read `scripts/hooks`
   (`scripts/install-hooks.sh` sets it; an unset value silently
   ran a stale `.git/hooks` copy until DCR-0018 caught it). If the
   installer exits non-zero refusing to touch a *different* `core.hooksPath`,
   that is by design — resolve it as the script's output says before
   releasing, never with `--no-verify`.

8. **Sibling consumers.** Every checkout below depends on `crates/transync`
   and `crates/transync-openai` by **path**, so it compiles against this
   working tree rather than a published version, and gets none of the warning
   a version bump gives:

   - `/Volumes/Common/QJoon/resp-translator`
   - `/Volumes/Common/QJoon/dynwebserver`

   For **each** of them: run `cargo check --workspace` there against the
   commit to be tagged, and record whether a migration was needed and any
   behavioral delta. v0.2.0's was an empty profile `target_language` now
   failing fast with `stable_code "internal"` — exactly the class of change a
   consumer that parses a profile at startup has to hear about. A green run
   is a result worth writing down too; both are recorded per consumer, not
   as one verdict.

   Both are separately-owned checkouts: a break that shows up here is
   *reported* to that consumer, never patched by editing it from this side.
   Their own thread caps (dynweb pins `--test-threads=4` for an external
   target dir) do not apply — `cargo check --workspace` is what this step
   asks for. Add a checkout to this list the day it grows a path dependency
   on this workspace, and drop one the day it stops; the release ritual can
   otherwise complete green while a consumer nobody listed is broken.

9. **Write the tallies down.** Suite counts (`N passed / N failed / N
   ignored`), CLI stub count, browser-suite count. `status.md`'s wave entries
   carry these numbers because a later regression is only detectable against
   a number someone recorded.

## C. Live-endpoint release gate (DCR-0015 Part C / OI-0030)

10. **Decide whether the gate is triggered — by inspection, not recall.** It
    is triggered if the release range touches `transync-openai`,
    `transync-core::llm::prompt`, the output schema (the Structured Output
    schema object and the `TranslationBatchResult` envelope the model has to
    fill), or batching:

    ```bash
    git diff --stat v<prev>..HEAD -- \
      crates/transync-openai \
      crates/transync-core/src/llm crates/transync-core/src/llm.rs \
      crates/transync-core/src/batch.rs \
      crates/transync-core/src/unit crates/transync-core/src/unit.rs
    ```

    When in doubt, run it. v0.2.0 ran the gate for a range whose only
    `llm::prompt` change was doc text, on the reasoning that the tag itself
    was the trigger.

    **For v0.4.0 the `git diff` above cannot be run at all.** After the
    2026-08-17 restart there is no `v<prev>` and no earlier commit to
    substitute for one — the history is a single root commit, so every range
    is empty and an empty diff would be an artifact of the object store
    rather than evidence about the release. **Decide by reading the
    `[Unreleased]` entries** for the same four surfaces (`transync-openai`,
    `transync-core::llm::prompt`, the output schema, batching) and, where an
    entry is ambiguous, the files themselves. **When in doubt, run the
    gate** — that instruction now carries the weight the diff used to. From
    the next release on, the `git diff --stat v<prev>..HEAD` form above
    applies again as written.

11. **Run it:** `OPENAI_API_KEY=… ./scripts/smoke-live-gate.sh` (default
    `all` — two tiny mini-model calls, one per API surface; `chat` /
    `responses` narrow it). The script supplies both gates the `#[ignore]`d
    tests require and refuses to run without a key. A failure means real
    drift — transport, auth, schema, or model behavior — because the
    assertions are structural (unit count, matching anchor counts,
    `fallback_source < total_units`, non-empty output differing from the
    source), not textual. If a default model identifier has been retired the
    failure surfaces at the provider: use `TRANSYNC_LIVE_SMOKE_CHAT_MODEL` /
    `TRANSYNC_LIVE_SMOKE_RESPONSES_MODEL` rather than reading it as a code
    regression.

12. **Record the outcome in the release's CHANGELOG section — date and both
    model names.** The v0.2.0 shape: “Release gate (OI-0030):
    `scripts/smoke-live-gate.sh` **PASS** on YYYY-MM-DD — 2/2
    machine-asserted live round-trips against `<chat-model>` (Chat
    Completions) and `<responses-model>` (Responses).” If the gate was *not*
    triggered, say so explicitly — an absent line is indistinguishable from a
    forgotten one.

## D. CHANGELOG

13. **Promote the section.** Rename `## [Unreleased]` to `## [X.Y.Z] -
    YYYY-MM-DD`, and open a fresh `## [Unreleased]` above it with a one-line
    placeholder naming the window state (v0.2.0 used “Nothing yet — v0.2.0
    closed the sanctioned breaking window…”). Keep the Keep a Changelog shape
    the file declares in its own header; entry sub-headings keep the file's
    existing `### Topic — headline (date)` style.

14. **Write the version preamble** directly under the new version heading:
    which window it closes or opens, and the live-gate record line from step
    12.

15. **Update the reference-link stanza at the bottom of the file.**
    `[Unreleased]` must compare `vX.Y.Z...HEAD`, and the new `[X.Y.Z]` needs
    its own definition — a compare URL `vPREV...vX.Y.Z` for every release
    after the first, a `releases/tag/` URL for the first. This step is on the
    list because v0.2.0 missed it: `[Unreleased]` still compared
    `v0.1.0...HEAD` and `[0.2.0]` had no definition at all, so the new
    version heading rendered as literal text until a follow-up commit fixed
    it.

    **v0.4.0's commit-URL form is spent, and it cost a second commit — which
    is the lesson worth keeping.** While the release was untagged, `[0.4.0]`
    was defined as a commit URL for the release-prep commit and `[Unreleased]`
    compared that sha to `HEAD`. A commit URL names the sha of the commit it
    ships in, which cannot be known before that commit exists, so the release
    committed with a `RELEASE_PREP_SHA` placeholder and substituted the real
    sha immediately afterward. **Expect two commits whenever a release ships
    untagged**; a tagged release does not have the problem, because the tag
    name is known in advance. Since the 2026-08-20 tag both entries were
    rewritten to their normal form — `[0.4.0]` is
    `https://github.com/QuietJoon/transync/releases/tag/v0.4.0` and
    `[Unreleased]` compares `v0.4.0...HEAD`. Step 16's invariant is unchanged:
    the heading still needs exactly one definition, and step 27 still requires
    it to render as a link. The stanza's existing `v0.1.0`–`v0.3.0` entries
    stay as written; they name refs those releases really carried when they
    shipped.

16. **Assert the invariant** before moving on: every `## [version]` heading
    has exactly one matching `[version]:` link definition, and every
    definition has a heading.

## E. Version bump

17. **Bump the version — and the five requirements that shadow it.** `version`
    under `[workspace.package]` in the root `Cargo.toml`. All eight members
    inherit it through `version.workspace = true`; there is no per-crate
    version to edit. (`crates/transync-wasm` additionally carries `publish =
    false`.)

    The root `[workspace.dependencies]` table now also carries the five
    internal members as `{ version = "X.Y.Z", path = "crates/…" }`
    (`transync-html`, `transync-syntax`, `transync-core`, `transync`,
    `transync-openai`) —
    the version requirement `cargo publish` demands, per step 19. **Those do
    not inherit**, so they move in the same edit; they sit directly under
    `[workspace.package]` for exactly that reason. A minor or major bump that
    forgets them fails the very next `cargo build` (the path member's version
    no longer satisfies the stale caret requirement), but a *patch* bump does
    not — `^0.2.0` still accepts `0.2.1` — so it would ship a published
    manifest understating what its sibling needs.

    `crates/transync/tests/workspace_publication.rs` is what checks them, so
    the build is not what you rely on:
    `every_internal_requirement_equals_the_workspace_version` reads the root
    manifest and every member's, fails on a requirement that no longer equals
    `[workspace.package] version`, and fails on a member that stopped
    inheriting it (a member pinning its own `version = "0.2.1"` against a
    stale `^0.2.0` resolves just as quietly). Run the suite after the bump —
    step 18 already asks for it — and a forgotten requirement is red rather
    than shipped.

18. **Re-run `cargo build --workspace` and the facade test suite after the
    bump.** The alignment map's `generator.version` is
    `env!("CARGO_PKG_VERSION")` and `crates/transync/tests/public_surface.rs`
    pins the two together (DCR-0017 M6), so the bump has to stay green, not
    merely compile. `Cargo.lock` **is** committed (owner decision 2026-08-06,
    `6cf4164`, reversing OI-0020's deferral), so the bump rewrites the six
    workspace entries in it and that diff belongs in the release-prep commit.
    The exact `wasm-bindgen = "=0.2.126"` pin in the root manifest is the
    control that does not depend on anyone honoring the lockfile, and must not
    be loosened as part of a release.

19. **Which members publish — and why the release still does not publish
    them.** Seven members are ordinary crates.io packages, and a first
    publication has to walk them in dependency order:

    | # | Member | Publishes | Why |
    |---|--------|-----------|-----|
    | 1 | `transync-html` | yes | the HTML mechanics layer, under the base crate (DCR-0032) |
    | 2 | `transync-syntax` | yes | the dependency-free base crate |
    | 3 | `transync-core` | yes | the pipeline |
    | 4 | `transync` | yes | the facade — the crate downstreams are meant to name |
    | 5 | `transync-openai` | yes | the default provider |
    | 6 | `transync-anthropic` | yes | the second provider (DCR-0029); depends on `transync` only, so it may publish any time after #4 |
    | 7 | `transync-cli` | yes | the reference binary (`cargo install transync-cli` installs `transync`) |
    | — | `transync-wasm` | **no** (`publish = false`) | a build target for the browser demo, not a library anyone depends on (ADR-0019) |

    That set is the reason the root `[workspace.dependencies]` table carries
    the five internal members with a `version` beside their `path` (step 17):
    packaging strips `path` and resolves the requirement from the registry, so
    a path-only internal dependency makes its package unpublishable outright
    (R0001-0040). `transync-wasm` needs no requirement of its own — it is
    private — but it takes the shared declaration anyway so there is exactly
    one place the edge is written. **`transync-anthropic` has no entry either,
    and that one is not an accident**: the table declares the edges members
    take *on each other*, and nothing in the workspace depends on the
    Anthropic adapter (the reference CLI stays OpenAI-backed by DCR-0029's
    scope). Its own outgoing edges — `transync`, plus the two dev-only ones —
    take their `version` from this table via `workspace = true`, which is all
    packaging needs; an entry naming a member nothing depends on is a
    requirement that will never be exercised, and
    `crates/transync/tests/workspace_publication.rs` fails it as
    `declared but unused`. The entry lands the day a member depends on it, in
    that same edit.

    The table above has a mechanical twin:
    `crates/transync/tests/workspace_publication.rs` carries the same roster as
    `PUBLISHED_MEMBERS` / `PRIVATE_MEMBERS` and asserts the two lists cover the
    workspace exactly. Publishing is cargo's default, so a member added without
    a decision would join this table by saying nothing at all; instead it turns
    the suite red until someone writes the decision down — here first, in the
    test second. A `publish` value the roster is not two-valued about (a
    registry allow-list, an explicit `publish = true`) is a hard failure there
    rather than a skipped member, and the same file welds the `workspace = true`
    rule this paragraph states.

    **Nothing in this workspace has been published to a registry**, and the
    one downstream consumer uses path dependencies, so no release so far has
    run `cargo publish`. Adding a real registry publish is a change to this
    checklist and needs its own record. What a release *does* owe is the dry
    run, so that publishability does not rot unnoticed between releases:

    ```bash
    cargo publish --dry-run --workspace
    ```

    It packages and verify-builds all seven and skips the `publish = false`
    member by itself. Budget for it: each package is verified in its **own**
    sandbox under `<target>/package/`, so the shared dependency graph is
    rebuilt from scratch once per package — 2m13s + 8m25s + 8m46s + 10m02s +
    11m10s ≈ 41 minutes over five packages on the 2026-08-06 run, and
    `transync-anthropic` (added 2026-08-10, DCR-0029) is a sixth sandbox of
    roughly `transync-openai`'s size on top of that. This is why it is a
    release step and not part of `scripts/smoke.sh`. Two more things to know
    before reading a failure:

    - **`--workspace` is the only form that works while the crates are
      unpublished.** A single-package `cargo publish --dry-run -p
      transync-core` fails with ``no matching package named `transync-syntax`
      found`` — with `path` stripped there is nothing on crates.io to resolve
      the requirement against. `--workspace` packages every member first and
      unpacks them into a temporary local registry
      (`<target>/package/tmp-registry`), which is what the log's `Unpacking
      transync-syntax v0.2.0 (registry …tmp-registry)` lines are. The same
      asymmetry applies to the real thing: the first publication is one
      `--workspace` invocation, not five `-p` ones.
    - **Uncommitted or untracked files under a package fail it** ("N files in
      the working directory contain changes that were not yet committed into
      git"). On the release-prep commit from step 1 there is nothing dirty;
      running it earlier takes `--allow-dirty`, which is a local convenience
      and never how the gate gets recorded.

## F. Records

20. **`docs/project/status.md`** — the workspace-version line gains the
    release date, the tag name, and the window state; the wave entry carries
    the tallies from step 9; the next-actions block strikes the release
    through and names what follows it.

    v0.4.0's carve-out here is spent. While the release was untagged the line
    read `v0.4.0 (untagged, owner decision 2026-08-13)` and pointed at *When it
    applies*, on the rule that a version line naming a ref `git rev-parse`
    cannot resolve is worse than one saying why there is none. That rule still
    governs; it simply no longer applies, because the ref resolves. The line
    now names the tag and the commit it points at, and says the tag was created
    after the release and makes no claim about ancestry — `git log v0.4.0`
    reaches one synthetic root, not the development that produced the release.

21. **`docs/project/phase-state.yaml`** — `last_updated` becomes the release
    marker (`YYYY-MM-DD-vX.Y.Z-released`) and the notes block records what
    the release did and which window is now open or closed.

22. **`docs/architecture/contracts.md`** — only if the release changed a
    stability rule or a documented surface. §0's curated table is welded to
    `crates/transync/tests/public_surface.rs`, so a table edit without the
    matching code edit fails the suite, and so does the reverse.

23. **Snapshot documents get dated appended notes, never in-place rewrites.**
    Any ADR or DCR whose decision the release amends (or whose follow-up it
    closes) gains a dated note at the bottom of the relevant section. Living
    documents — this file, `status.md`, `contracts.md`, `mvp-scope.md`, the
    Developer Guide — are edited in place.

## G. Commit and tag

24. **One coherent release-prep commit.** v0.2.0 used `Release prep for
    vX.Y.Z`, carrying the CHANGELOG promotion, the version bump, the surface
    changes, and the record updates together. The pre-commit hook must run:
    never `--no-verify`.

25. **Annotated tag, always:**

    ```bash
    git tag -a vX.Y.Z -m "transync vX.Y.Z — <headline>"
    ```

    **v0.4.0's skip is spent.** It was skipped on 2026-08-13's decision and
    then tagged on 2026-08-20 when the owner reversed it (*When it applies*),
    so this step has no exception left in it. A release that tags late tags the
    release-prep commit from step 24 — the same commit the CHANGELOG names —
    and says in the tag message why it is late, because a tag whose date
    trails its release is a question someone will ask.

    **One tag survives**: `git tag -l` prints `v0.4.0` and nothing else. The
    v0.1.0, v0.2.0 and v0.3.0 tag objects went with the history restarted on
    2026-08-10 and were deliberately not recreated, because the commits they
    pointed at are gone too (`docs/project/git-history-loss-2026-08-10.md`
    holds their archived hashes); v0.4.0's was created on 2026-08-20 by the
    reversal above. The rule they were evidence for is unchanged
    for every tag this repository creates from now on: annotated, never
    lightweight. A lightweight tag is just a moving pointer: no tagger, no
    date, no message, and nothing that records what the release was.

26. **Verify the tag is annotated and says what you meant:**

    ```bash
    git for-each-ref --format='%(refname:short) %(objecttype) %(taggerdate:short)' refs/tags
    git tag -l -n20 vX.Y.Z
    ```

    `%(objecttype)` must read `tag`, not `commit`.

    **v0.4.0's inverted form is spent.** While the release was untagged this
    check ran backwards — the first command had to print *nothing*, confirming
    both that step 25 was skipped on purpose and that no tag had been
    resurrected by accident. It was run in that form and passed. Since the
    2026-08-20 reversal the normal form applies: `v0.4.0 tag 2026-08-20` is
    what it prints, and `%(objecttype)` reading `tag` is what it proves.

## H. After the tag

27. **Re-read the top of the rendered CHANGELOG.** The new version heading
    must render as a link (the step-15 failure mode is a heading that renders
    as literal text), and `[Unreleased]` must hold only its placeholder line
    and compare against the new tag. v0.4.0's stand-in — the release-prep
    commit URL from step 15 — is spent; both entries name the tag now.

28. **Push, then publish.** Since 2026-08-19 this clone has one remote, named
    **`backup`** — `/Volumes/Common/git-backup/transync.git`, a bare mirror on
    the same machine, with `master` tracking `backup/master`. `git push backup
    master` after the release-prep commit, and after the tag when there is one
    (`git push backup vX.Y.Z`). That push is not publication; it is the
    redundancy whose absence made the 2026-08-17 loss unrecoverable
    (`docs/project/git-history-loss-2026-08-17.md`), and it is the reason the
    2026-08-19 recurrence cost nothing.

    **Publication is still unwired.** The CHANGELOG's compare URLs name
    `github.com/QuietJoon/transync`, the `repository` field of the root
    manifest, and no remote points there. Those links resolve only once the
    commit — and the tag, when there is one — reach it. v0.4.0 has both: the
    release-prep commit and, since 2026-08-20, the `v0.4.0` tag, whose
    `releases/tag/` URL is what starts resolving if that remote is ever wired
    up. Both are on the `backup` mirror today, which is redundancy, not
    publication.

    **Step 15's commit-URL form costs a second commit, by construction** — the
    reason to prefer tagging even when a release's history is unusual. The URL
    names the release-prep commit's own sha, which cannot be known before that
    commit exists, so v0.4.0 committed with a `RELEASE_PREP_SHA` placeholder
    and substituted the real sha immediately afterward. There is no one-commit
    form of it. A tag-based release does not have the problem, because the tag
    name is known in advance. Expect two commits whenever a release ships
    untagged.

29. **Feed the checklist back.** If the release exposed a missing step, a
    gate that did not exist, or an ordering that bit, add it here in the same
    session. That is the entire reason this document exists rather than the
    ritual living in someone's memory.
