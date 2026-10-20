//! The repository's own shell scripts, driven as subprocesses under shims.
//!
//! Three guards live in `scripts/` rather than in a crate, and each protects
//! something no in-process test can reach:
//!
//! - [`workdir_overlap`] — the read-side rule in `scripts/lib/workdir-guard.sh`
//!   (R0012-0003). `smoke-live.sh`, `smoke.sh` and `test-browser.sh` delete
//!   their workdir before the run reads its input, profile, prompt file and
//!   checkout, so a workdir holding any of them is refused first.
//! - [`commit_snapshot`] — `scripts/lib/commit-snapshot-guard.sh` as wired into
//!   `scripts/hooks/pre-commit` (R0012-0016). The gates read the working tree
//!   and the commit records the index, so a commit whose gate inputs disagree
//!   with the tree the gates read is refused before any gate runs.
//! - [`live_gate`] — `scripts/smoke-live-gate.sh` names every leg's test
//!   exactly and checks the whole inventory before the first paid call
//!   (R0012-0017), and shape-checks an override only for the legs that read it
//!   (R0012-0027).
//!
//! Every run clears the environment, so `cargo` (and `rustup` or `pnpm` where
//! a script probes them) is a stand-in that records its arguments: an
//! exported bash function, which every bash the scripts start inherits.
//! Functions rather than shim files because macOS assesses each newly written
//! executable on its first exec, which costs from a tenth of a second to tens
//! of seconds apiece; nothing in this file writes an executable that runs. A
//! tripwire file of the same name still sits first on PATH, so a lookup that
//! bypassed the function fails loudly instead of reaching a real `cargo`, and
//! before any script runs the harness checks that `cargo` is the function.
//! Nothing here builds, opens a socket or reaches a provider.

#![cfg(unix)]

mod common;

use common::ScratchDir;
use std::ffi::OsString;
use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root should resolve from CARGO_MANIFEST_DIR/../..")
}

/// The first directory on the inherited PATH holding an executable `name`.
/// Only `bash` and `git` are located this way; nothing else is inherited.
fn tool_dir(name: &str) -> PathBuf {
    let path = std::env::var_os("PATH").unwrap_or_default();
    for dir in std::env::split_paths(&path) {
        if let Ok(meta) = fs::metadata(dir.join(name))
            && meta.is_file()
            && meta.permissions().mode() & 0o111 != 0
        {
            return dir;
        }
    }
    panic!("`{name}` must be on PATH to drive the repository's scripts");
}

fn write_executable(path: &Path, body: &str) {
    fs::write(path, body).expect("shim is writable");
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("shim is chmod-able");
}

fn transcript(out: &Output) -> String {
    format!(
        "status: {:?}\n--- stdout ---\n{}\n--- stderr ---\n{}",
        out.status.code(),
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

fn stderr_and_stdout(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

/// One scratch directory, one cleared environment, and the stand-ins it
/// exports.
struct Harness {
    scratch: ScratchDir,
    bin: PathBuf,
    home: PathBuf,
    bash: PathBuf,
    functions: Vec<(String, String)>,
}

impl Harness {
    fn new(prefix: &str) -> Harness {
        let scratch = ScratchDir::new(prefix);
        let bin = scratch.join("bin");
        let home = scratch.join("home");
        fs::create_dir_all(&bin).expect("shim dir");
        fs::create_dir_all(&home).expect("home dir");
        let bash = tool_dir("bash").join("bash");
        Harness {
            scratch,
            bin,
            home,
            bash,
            functions: Vec::new(),
        }
    }

    fn path(&self) -> &Path {
        self.scratch.path()
    }

    /// Export `name() { body }` to every bash the harness starts, and put a
    /// tripwire of the same name first on PATH. The body runs inside the
    /// calling script's shell, so it `return`s rather than `exit`s and
    /// declares every variable `local`.
    fn shim(&mut self, name: &str, body: &str) {
        write_executable(
            &self.bin.join(name),
            &format!(
                "#!/bin/sh\necho \"tripwire: $0 ran; the exported {name} function was bypassed\" >&2\nexit 99\n"
            ),
        );
        self.functions.push((name.to_string(), body.to_string()));
    }

    fn path_var(&self) -> OsString {
        std::env::join_paths([
            self.bin.clone(),
            tool_dir("bash"),
            tool_dir("git"),
            PathBuf::from("/usr/bin"),
            PathBuf::from("/bin"),
        ])
        .expect("PATH entries contain no separator")
    }

    /// `program` with the environment cleared down to PATH and HOME.
    fn command(&self, program: &Path) -> Command {
        let mut cmd = Command::new(program);
        cmd.env_clear()
            .env("PATH", self.path_var())
            .env("HOME", &self.home);
        for (name, body) in &self.functions {
            cmd.env(format!("BASH_FUNC_{name}%%"), format!("() {{\n{body}\n}}"));
        }
        cmd
    }

    /// Stop unless a bash under this environment sees `cargo` as the exported
    /// stand-in, so no script under test can reach a real build or a
    /// provider.
    fn assert_cargo_is_the_stand_in(&self) {
        let probe = self
            .command(&self.bash)
            .args(["-c", "type -t cargo"])
            .output()
            .expect("bash runs");
        assert_eq!(
            String::from_utf8_lossy(&probe.stdout).trim(),
            "function",
            "cargo must be the exported stand-in before any script runs: {}",
            transcript(&probe)
        );
    }

    /// A bash running `script` under the cleared environment.
    fn script(&self, script: &Path) -> Command {
        self.assert_cargo_is_the_stand_in();
        let mut cmd = self.command(&self.bash);
        cmd.arg(script);
        cmd
    }

    fn log(&self, name: &str) -> String {
        fs::read_to_string(self.path().join(name)).unwrap_or_default()
    }
}

// ---------------------------------------------------------------------------
mod workdir_overlap {
    //! R0012-0003: a deletable workdir must not hold anything the run reads.

    use super::*;

    /// The cargo shim's exit status: reaching it means the script got past its
    /// deletion step to the first build.
    const REACHED_CARGO: i32 = 97;
    const SOURCE: &str = "# A source the operator cares about\n";

    fn harness(prefix: &str) -> Harness {
        let mut h = Harness::new(prefix);
        let log = h.path().join("cargo.log");
        h.shim(
            "cargo",
            &format!(
                "printf '%s\\n' \"$*\" >> '{}'\nreturn {REACHED_CARGO}",
                log.display()
            ),
        );
        h.shim("pnpm", "return 0");
        h
    }

    /// `<scratch>/wd`, stamped as an earlier run's own workdir.
    fn marked_workdir(h: &Harness) -> PathBuf {
        let wd = h.path().join("wd");
        fs::create_dir_all(&wd).unwrap();
        fs::write(wd.join(".transync-workdir"), "marker\n").unwrap();
        wd
    }

    fn file(path: &Path, bytes: &str) -> PathBuf {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
        path.to_path_buf()
    }

    fn smoke_live(h: &Harness, script: &Path, env: &[(&str, &Path)]) -> Output {
        let mut cmd = h.script(script);
        cmd.env("OPENAI_API_KEY", "sk-dummy-never-sent");
        for (key, value) in env {
            cmd.env(key, value);
        }
        cmd.output().expect("smoke-live.sh runs")
    }

    fn production(name: &str) -> PathBuf {
        repo_root().join("scripts").join(name)
    }

    fn assert_refused(h: &Harness, out: &Output, naming: &str) {
        assert_eq!(out.status.code(), Some(1), "{}", transcript(out));
        let text = stderr_and_stdout(out);
        assert!(
            text.contains("refusing to rm -rf") && text.contains(naming),
            "the refusal must name the read path {naming}:\n{}",
            transcript(out)
        );
        assert!(
            h.log("cargo.log").is_empty(),
            "a refused run must stop before the build:\n{}",
            h.log("cargo.log")
        );
    }

    #[test]
    fn an_input_inside_the_workdir_is_refused_before_anything_is_deleted() {
        let h = harness("transync-overlap-input");
        let wd = marked_workdir(&h);
        let input = file(&wd.join("source.md"), SOURCE);
        let out = smoke_live(
            &h,
            &production("smoke-live.sh"),
            &[
                ("TRANSYNC_LIVE_WORKDIR", &wd),
                ("TRANSYNC_LIVE_INPUT", &input),
            ],
        );
        assert_refused(&h, &out, "source.md");
        assert_eq!(fs::read_to_string(&input).unwrap(), SOURCE);
    }

    #[test]
    fn an_input_linked_into_the_workdir_is_refused() {
        let h = harness("transync-overlap-link");
        let wd = marked_workdir(&h);
        let target = file(&wd.join("source.md"), SOURCE);
        let link = h.path().join("elsewhere/link.md");
        fs::create_dir_all(link.parent().unwrap()).unwrap();
        symlink(&target, &link).unwrap();
        let out = smoke_live(
            &h,
            &production("smoke-live.sh"),
            &[
                ("TRANSYNC_LIVE_WORKDIR", &wd),
                ("TRANSYNC_LIVE_INPUT", &link),
            ],
        );
        assert_refused(&h, &out, "link.md");
        assert_eq!(fs::read_to_string(&target).unwrap(), SOURCE);
    }

    #[test]
    fn a_profile_or_prompt_file_inside_the_workdir_is_refused() {
        for (knob, name) in [
            ("TRANSYNC_LIVE_PROFILE", "p.toml"),
            ("TRANSYNC_LIVE_SYSTEM_PROMPT_FILE", "prompt.txt"),
        ] {
            let h = harness("transync-overlap-knob");
            let wd = marked_workdir(&h);
            let input = file(&h.path().join("in/source.md"), SOURCE);
            let read = file(&wd.join(name), "kept\n");
            let out = smoke_live(
                &h,
                &production("smoke-live.sh"),
                &[
                    ("TRANSYNC_LIVE_WORKDIR", &wd),
                    ("TRANSYNC_LIVE_INPUT", &input),
                    (knob, &read),
                ],
            );
            assert_refused(&h, &out, name);
            assert_eq!(fs::read_to_string(&read).unwrap(), "kept\n", "{knob}");
        }
    }

    /// Identity, not spelling: the workdir reached through a symlinked
    /// ancestor, through `..`, and — where the volume folds case — through
    /// another letter case is still the workdir.
    #[test]
    fn an_input_reached_through_another_spelling_of_the_workdir_is_refused() {
        let h = harness("transync-overlap-spelling");
        let wd = marked_workdir(&h);
        let input = file(&wd.join("source.md"), SOURCE);
        symlink(h.path(), h.path().join("alias")).unwrap();
        fs::create_dir_all(h.path().join("in")).unwrap();

        let mut spellings = vec![
            h.path().join("alias/wd/source.md"),
            h.path().join("in/../wd/source.md"),
        ];
        let folds = {
            fs::write(h.path().join("CaseProbe"), "").unwrap();
            h.path().join("caseprobe").exists()
        };
        if folds {
            spellings.push(h.path().join("WD/SOURCE.md"));
        }
        for spelling in spellings {
            let out = smoke_live(
                &h,
                &production("smoke-live.sh"),
                &[
                    ("TRANSYNC_LIVE_WORKDIR", &wd),
                    ("TRANSYNC_LIVE_INPUT", &spelling),
                ],
            );
            assert_refused(&h, &out, &spelling.display().to_string());
            assert_eq!(fs::read_to_string(&input).unwrap(), SOURCE);
        }
    }

    /// No false positive: an input beside the workdir reaches the build, and
    /// the workdir is still wiped and re-marked.
    #[test]
    fn an_input_beside_the_workdir_reaches_the_build() {
        let h = harness("transync-overlap-beside");
        let wd = marked_workdir(&h);
        let stale = file(&wd.join("stale.txt"), "old run\n");
        let input = file(&h.path().join("in/source.md"), SOURCE);
        let out = smoke_live(
            &h,
            &production("smoke-live.sh"),
            &[
                ("TRANSYNC_LIVE_WORKDIR", &wd),
                ("TRANSYNC_LIVE_INPUT", &input),
            ],
        );
        assert_eq!(
            out.status.code(),
            Some(REACHED_CARGO),
            "{}",
            transcript(&out)
        );
        assert_eq!(fs::read_to_string(&input).unwrap(), SOURCE);
        assert!(!stale.exists(), "the workdir is still wiped");
        assert!(wd.join(".transync-workdir").is_file(), "and re-marked");
    }

    /// A workdir named as a symlink without a trailing slash: `rm -rf`
    /// removes the link, so an input in the link's target is not at stake.
    #[test]
    fn a_workdir_named_as_a_link_puts_only_the_link_at_stake() {
        let h = harness("transync-overlap-wdlink");
        let input = file(&h.path().join("real/source.md"), SOURCE);
        let link = h.path().join("link");
        symlink(h.path().join("real"), &link).unwrap();
        let out = smoke_live(
            &h,
            &production("smoke-live.sh"),
            &[
                ("TRANSYNC_LIVE_WORKDIR", &link),
                ("TRANSYNC_LIVE_INPUT", &input),
            ],
        );
        assert_eq!(
            out.status.code(),
            Some(REACHED_CARGO),
            "{}",
            transcript(&out)
        );
        assert_eq!(fs::read_to_string(&input).unwrap(), SOURCE);
        let meta = fs::symlink_metadata(&link).unwrap();
        assert!(meta.is_dir(), "the link was replaced by a fresh workdir");
    }

    /// The same link named with a trailing slash: `rm -rf` follows it and
    /// empties the target, so the input in the target is at stake.
    #[test]
    fn a_workdir_named_as_a_link_with_a_trailing_slash_is_refused() {
        let h = harness("transync-overlap-slash");
        let input = file(&h.path().join("real/source.md"), SOURCE);
        symlink(h.path().join("real"), h.path().join("link")).unwrap();
        let slashed = PathBuf::from(format!("{}/", h.path().join("link").display()));
        let out = smoke_live(
            &h,
            &production("smoke-live.sh"),
            &[
                ("TRANSYNC_LIVE_WORKDIR", &slashed),
                ("TRANSYNC_LIVE_INPUT", &input),
            ],
        );
        assert_refused(&h, &out, "source.md");
        assert_eq!(fs::read_to_string(&input).unwrap(), SOURCE);
    }

    /// An input spelled through the workdir link stops resolving once the
    /// link is removed, so it is refused although its bytes would survive.
    #[test]
    fn an_input_spelled_through_the_workdir_link_is_refused() {
        let h = harness("transync-overlap-through");
        file(&h.path().join("real/source.md"), SOURCE);
        let link = h.path().join("link");
        symlink(h.path().join("real"), &link).unwrap();
        let out = smoke_live(
            &h,
            &production("smoke-live.sh"),
            &[
                ("TRANSYNC_LIVE_WORKDIR", &link),
                ("TRANSYNC_LIVE_INPUT", &link.join("source.md")),
            ],
        );
        assert_refused(&h, &out, "source.md");
        assert!(
            fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink()
        );
    }

    /// A checkout under the workdir is refused by every script that wipes its
    /// whole workdir. Each runs from a copy of the production script placed
    /// inside the workdir, so its own checkout is the read path at stake.
    #[test]
    fn a_checkout_inside_the_workdir_is_refused_by_every_wiping_script() {
        for (script, knob) in [
            ("smoke.sh", "TRANSYNC_SMOKE_WORKDIR"),
            ("test-browser.sh", "TRANSYNC_FIXTURE_WORKDIR"),
            ("smoke-live.sh", "TRANSYNC_LIVE_WORKDIR"),
        ] {
            let h = harness("transync-overlap-checkout");
            let wd = marked_workdir(&h);
            let checkout = wd.join("checkout");
            fs::create_dir_all(checkout.join("scripts/lib")).unwrap();
            fs::copy(production(script), checkout.join("scripts").join(script)).unwrap();
            for lib in fs::read_dir(repo_root().join("scripts/lib")).unwrap() {
                let lib = lib.unwrap().path();
                fs::copy(
                    &lib,
                    checkout.join("scripts/lib").join(lib.file_name().unwrap()),
                )
                .unwrap();
            }
            // What test-browser.sh checks before its deletion step, so the
            // unguarded script would reach `rm -rf` rather than stop early.
            file(
                &checkout.join("crates/transync/tests/fixtures/scn-14-full.md"),
                SOURCE,
            );
            let input = file(&h.path().join("in/source.md"), SOURCE);
            let out = smoke_live(
                &h,
                &checkout.join("scripts").join(script),
                &[(knob, &wd), ("TRANSYNC_LIVE_INPUT", &input)],
            );
            assert_refused(&h, &out, "checkout");
            assert!(
                checkout.join("scripts").join(script).is_file(),
                "{script} must leave the checkout under its workdir intact"
            );
        }
    }

    /// The production layout — the checkout outside the workdir — still
    /// reaches the build for the two scripts that only read the checkout.
    #[test]
    fn the_production_layout_still_reaches_the_build() {
        for (script, knob) in [
            ("smoke.sh", "TRANSYNC_SMOKE_WORKDIR"),
            ("test-browser.sh", "TRANSYNC_FIXTURE_WORKDIR"),
        ] {
            let h = harness("transync-overlap-layout");
            let wd = marked_workdir(&h);
            let out = smoke_live(&h, &production(script), &[(knob, &wd)]);
            assert_eq!(
                out.status.code(),
                Some(REACHED_CARGO),
                "{script}: {}",
                transcript(&out)
            );
            assert!(wd.join(".transync-workdir").is_file(), "{script}");
        }
    }

    /// The library alone: empty read paths are skipped, a workdir that does
    /// not exist puts nothing at stake, and a symlink loop on a read path is a
    /// refusal rather than a hang.
    #[test]
    fn the_read_side_guard_skips_what_cannot_be_at_stake_and_terminates() {
        let h = harness("transync-overlap-lib");
        let lib = repo_root().join("scripts/lib/workdir-guard.sh");
        let guard = |workdir: &Path, read: &Path| -> Output {
            h.command(&h.bash)
                .args([
                    "-c",
                    "source \"$1\"; transync_guard_read_side t T_VAR \"$2\" '' \"$3\"; echo passed",
                    "_",
                ])
                .arg(&lib)
                .arg(workdir)
                .arg(read)
                .output()
                .expect("bash runs")
        };
        let input = file(&h.path().join("in/source.md"), SOURCE);

        let out = guard(&h.path().join("absent"), &input);
        assert!(
            String::from_utf8_lossy(&out.stdout).contains("passed"),
            "{}",
            transcript(&out)
        );

        let wd = marked_workdir(&h);
        symlink(h.path().join("loop-b"), h.path().join("loop-a")).unwrap();
        symlink(h.path().join("loop-a"), h.path().join("loop-b")).unwrap();
        let out = guard(&wd, &h.path().join("loop-a"));
        assert_eq!(out.status.code(), Some(1), "{}", transcript(&out));
        assert!(
            String::from_utf8_lossy(&out.stderr).contains("cannot resolve"),
            "{}",
            transcript(&out)
        );
    }
}

// ---------------------------------------------------------------------------
mod commit_snapshot {
    //! R0012-0016: the hook refuses a commit whose gate inputs disagree with
    //! the tree its gates read, and warns when the tree is a superset of it.

    use super::*;

    struct Repo {
        h: Harness,
        root: PathBuf,
        git: PathBuf,
    }

    impl Repo {
        /// A scratch checkout carrying committed copies of the production
        /// hook and its libraries, with `core.hooksPath` naming the
        /// production hook directory.
        fn new(prefix: &str) -> Repo {
            let mut h = Harness::new(prefix);
            let log = h.path().join("cargo.log");
            h.shim(
                "cargo",
                &format!("printf '%s\\n' \"$*\" >> '{}'\nreturn 0", log.display()),
            );
            h.shim(
                "rustup",
                "case \"$*\" in *'target list'*) echo wasm32-unknown-unknown ;; esac\nreturn 0",
            );
            let root = h.path().join("repo");
            fs::create_dir_all(root.join("scripts/hooks")).unwrap();
            fs::create_dir_all(root.join("scripts/lib")).unwrap();
            fs::copy(
                repo_root().join("scripts/hooks/pre-commit"),
                root.join("scripts/hooks/pre-commit"),
            )
            .unwrap();
            for lib in fs::read_dir(repo_root().join("scripts/lib")).unwrap() {
                let lib = lib.unwrap().path();
                fs::copy(
                    &lib,
                    root.join("scripts/lib").join(lib.file_name().unwrap()),
                )
                .unwrap();
            }
            let repo = Repo {
                git: tool_dir("git").join("git"),
                h,
                root,
            };
            repo.write("Cargo.toml", "[workspace]\nmembers = []\n");
            repo.write("crates/demo/src/lib.rs", "pub fn a() {}\n");
            repo.write("crates/demo/src/c.rs", "pub fn c() {}\n");
            repo.write("docs/notes.md", "notes\n");
            repo.write(".gitignore", "crates/demo/scratch/\n");
            repo.git_ok(&["init", "-q"]);
            repo.git_ok(&["add", "-A"]);
            repo.git_ok(&["commit", "-q", "-m", "init"]);
            // The production hook runs from where it lives: it works on the
            // checkout `git rev-parse --show-toplevel` names, and sourcing the
            // scratch copies of its libraries from there is its own rule. The
            // scratch copy of the hook is the gate input under test, never
            // exec'd — a fresh executable per test would pay macOS's
            // first-exec assessment each time.
            let hooks = repo_root().join("scripts/hooks");
            repo.git_ok(&["config", "core.hooksPath", hooks.to_str().unwrap()]);
            repo
        }

        fn write(&self, rel: &str, text: &str) {
            let path = self.root.join(rel);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, text).unwrap();
        }

        fn append(&self, rel: &str, text: &str) {
            let path = self.root.join(rel);
            let mut body = fs::read_to_string(&path).unwrap();
            body.push_str(text);
            fs::write(path, body).unwrap();
        }

        fn git(&self, args: &[&str]) -> Output {
            self.h
                .command(&self.git)
                .current_dir(&self.root)
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .env("GIT_CONFIG_GLOBAL", "/dev/null")
                .env("GIT_AUTHOR_NAME", "t")
                .env("GIT_AUTHOR_EMAIL", "t@example.invalid")
                .env("GIT_COMMITTER_NAME", "t")
                .env("GIT_COMMITTER_EMAIL", "t@example.invalid")
                .args(["-c", "init.defaultBranch=master"])
                .args(args)
                .output()
                .expect("git runs")
        }

        fn git_ok(&self, args: &[&str]) -> String {
            let out = self.git(args);
            assert!(out.status.success(), "git {args:?}: {}", transcript(&out));
            String::from_utf8_lossy(&out.stdout).into_owned()
        }

        fn head(&self) -> String {
            self.git_ok(&["rev-parse", "HEAD"])
        }

        /// `git commit <args>`, which runs the hook, after proving the hook's
        /// `cargo` is the stand-in.
        fn commit(&self, args: &[&str]) -> Output {
            self.h.assert_cargo_is_the_stand_in();
            let mut full = vec!["commit", "-q"];
            full.extend_from_slice(args);
            self.git(&full)
        }

        fn gates_ran(&self) -> bool {
            self.h.log("cargo.log").contains("fmt --all -- --check")
        }
    }

    fn assert_refused(repo: &Repo, before: &str, out: &Output, path: &str) {
        let text = stderr_and_stdout(out);
        assert!(!out.status.success(), "{}", transcript(out));
        assert!(
            text.contains("NOT CHECKABLE") && text.contains(path),
            "the refusal must name {path}:\n{}",
            transcript(out)
        );
        assert!(
            !repo.gates_ran(),
            "the refusal comes before any gate runs:\n{}",
            repo.h.log("cargo.log")
        );
        assert_eq!(repo.head(), before, "nothing was committed");
    }

    fn assert_committed(repo: &Repo, before: &str, out: &Output) {
        assert!(out.status.success(), "{}", transcript(out));
        assert!(
            !stderr_and_stdout(out).contains("NOT CHECKABLE"),
            "{}",
            transcript(out)
        );
        assert!(repo.gates_ran(), "the gates ran");
        assert_ne!(repo.head(), before, "the commit landed");
    }

    /// The forgot-to-re-stage case: `cargo fmt` (here, any edit) after
    /// `git add` leaves the staged blob unchecked.
    #[test]
    fn a_gate_input_staged_then_edited_again_is_refused() {
        let repo = Repo::new("transync-snapshot-restage");
        let before = repo.head();
        repo.append("crates/demo/src/lib.rs", "pub fn b() {}\n");
        repo.git_ok(&["add", "crates/demo/src/lib.rs"]);
        repo.append("crates/demo/src/lib.rs", "// formatted afterwards\n");
        let out = repo.commit(&["-m", "x"]);
        assert_refused(&repo, &before, &out, "crates/demo/src/lib.rs");
    }

    /// The forgot-to-add-a-module case: the build reaches an untracked file
    /// the commit would not carry.
    #[test]
    fn an_untracked_gate_input_is_refused() {
        let repo = Repo::new("transync-snapshot-untracked");
        let before = repo.head();
        repo.append("crates/demo/src/lib.rs", "mod b;\n");
        repo.write("crates/demo/src/b.rs", "pub fn b() {}\n");
        repo.git_ok(&["add", "crates/demo/src/lib.rs"]);
        let out = repo.commit(&["-m", "x"]);
        assert_refused(&repo, &before, &out, "crates/demo/src/b.rs");
    }

    /// The hook that runs is the working copy, so it is a gate input too.
    #[test]
    fn the_hook_itself_staged_then_edited_is_refused() {
        let repo = Repo::new("transync-snapshot-hook");
        let before = repo.head();
        repo.append("scripts/hooks/pre-commit", "# staged\n");
        repo.git_ok(&["add", "scripts/hooks/pre-commit"]);
        repo.append("scripts/hooks/pre-commit", "# and edited again\n");
        let out = repo.commit(&["-m", "x"]);
        assert_refused(&repo, &before, &out, "scripts/hooks/pre-commit");
    }

    /// `git commit -a` hands the hook a temporary index holding the working
    /// tree, so the same edits are checkable there.
    #[test]
    fn commit_all_is_checkable() {
        let repo = Repo::new("transync-snapshot-all");
        let before = repo.head();
        repo.append("crates/demo/src/lib.rs", "pub fn b() {}\n");
        repo.git_ok(&["add", "crates/demo/src/lib.rs"]);
        repo.append("crates/demo/src/lib.rs", "// formatted afterwards\n");
        let out = repo.commit(&["-a", "-m", "x"]);
        assert_committed(&repo, &before, &out);
    }

    /// `git commit <paths>` judges its own temporary index: the named path is
    /// clean there, and a divergent gate input the commit leaves out is a
    /// warning, not a refusal.
    #[test]
    fn a_commit_of_named_paths_judges_its_own_index() {
        let repo = Repo::new("transync-snapshot-only");
        let before = repo.head();
        repo.append("crates/demo/src/lib.rs", "pub fn b() {}\n");
        repo.git_ok(&["add", "crates/demo/src/lib.rs"]);
        repo.append("crates/demo/src/lib.rs", "// not in this commit\n");
        repo.append("crates/demo/src/c.rs", "pub fn d() {}\n");
        let out = repo.commit(&["-m", "x", "--", "crates/demo/src/c.rs"]);
        assert_committed(&repo, &before, &out);
        let text = stderr_and_stdout(&out);
        assert!(
            text.contains("SUPERSET") && text.contains("crates/demo/src/lib.rs"),
            "the left-out divergence is warned about:\n{}",
            transcript(&out)
        );
        let changed = repo.git_ok(&["show", "--name-only", "--format=", "HEAD"]);
        assert_eq!(changed.trim(), "crates/demo/src/c.rs");
    }

    /// A gate input this commit does not touch, edited in the working tree:
    /// the gates read it, so the verdict covers more than the commit — said,
    /// not refused.
    #[test]
    fn an_unstaged_gate_input_outside_the_commit_is_a_warning() {
        let repo = Repo::new("transync-snapshot-warn");
        let before = repo.head();
        repo.append("crates/demo/src/c.rs", "pub fn d() {}\n");
        repo.append("docs/notes.md", "more\n");
        repo.git_ok(&["add", "docs/notes.md"]);
        let out = repo.commit(&["-m", "x"]);
        assert_committed(&repo, &before, &out);
        let text = stderr_and_stdout(&out);
        assert!(
            text.contains("SUPERSET") && text.contains("crates/demo/src/c.rs"),
            "{}",
            transcript(&out)
        );
    }

    /// No gate reads docs, an ignored file, or untracked JavaScript outside
    /// `crates/` (the JS gate's corpus is what git tracks), so none of them
    /// blocks a commit or earns a warning.
    #[test]
    fn divergence_outside_the_gate_inputs_passes_silently() {
        let repo = Repo::new("transync-snapshot-docs");
        let before = repo.head();
        repo.append("docs/notes.md", "staged\n");
        repo.git_ok(&["add", "docs/notes.md"]);
        repo.append("docs/notes.md", "and edited again\n");
        repo.write("crates/demo/scratch/out.rs", "ignored\n");
        repo.write("research/notes.js", "untracked();\n");
        repo.append("crates/demo/src/c.rs", "pub fn d() {}\n");
        repo.git_ok(&["add", "crates/demo/src/c.rs"]);
        let out = repo.commit(&["-m", "x"]);
        assert_committed(&repo, &before, &out);
        let text = stderr_and_stdout(&out);
        assert!(!text.contains("SUPERSET"), "{}", transcript(&out));
    }
}

// ---------------------------------------------------------------------------
mod live_gate {
    //! R0012-0017: every leg names its test exactly and the whole inventory is
    //! checked before the first paid run. R0012-0027: an override is checked
    //! only for the legs that read it.

    use super::*;

    /// Surface, provider crate, and the live test that surface certifies.
    const LEGS: &[(&str, &str, &str)] = &[
        ("chat", "transync-openai", "live_chat_surface_round_trip"),
        (
            "responses",
            "transync-openai",
            "live_responses_surface_round_trip",
        ),
        (
            "anthropic",
            "transync-anthropic",
            "live_messages_api_round_trip",
        ),
        ("google-vertex", "transync-google", "vertex_live_smoke"),
        (
            "google-vertex-express",
            "transync-google",
            "vertex_express_live_smoke",
        ),
        (
            "google-developer",
            "transync-google",
            "developer_live_smoke",
        ),
    ];

    const PACKAGES: &[&str] = &["transync-openai", "transync-anthropic", "transync-google"];

    /// The ignored tests a provider's `tests/live_smoke.rs` declares, read the
    /// way `--ignored --list` would select them: a `fn` whose attribute block
    /// carries both a test attribute and `#[ignore`.
    fn ignored_live_tests(package: &str) -> Vec<String> {
        let path = repo_root()
            .join("crates")
            .join(package)
            .join("tests/live_smoke.rs");
        let text = fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path:?}: {e}"));
        let (mut is_test, mut ignored, mut found) = (false, false, Vec::new());
        for line in text.lines().map(str::trim) {
            if line.starts_with("#[test]") || line.starts_with("#[tokio::test") {
                is_test = true;
            } else if line.starts_with("#[ignore") {
                ignored = true;
            } else if let Some(rest) = line
                .strip_prefix("async fn ")
                .or_else(|| line.strip_prefix("fn "))
            {
                if is_test && ignored {
                    found.push(rest.split('(').next().unwrap().trim().to_string());
                }
                is_test = false;
                ignored = false;
            } else if !(line.starts_with("#[") || line.starts_with("///")) {
                is_test = false;
                ignored = false;
            }
        }
        found
    }

    struct Gate {
        h: Harness,
    }

    impl Gate {
        /// A cargo shim that answers `--list` from an inventory file per
        /// package, filtering the way libtest does (substring, or equality
        /// under `--exact`), and records every list and run.
        fn new(prefix: &str) -> Gate {
            let mut h = Harness::new(prefix);
            let state = h.path().display().to_string();
            h.shim(
                "cargo",
                &format!(
                    r#"local state='{state}' pkg='' filter='' list=0 exact=0 after=0 prev='' a name n
shift
for a in "$@"; do
  if [ "$after" = 1 ]; then
    case "$a" in --list) list=1 ;; --exact) exact=1 ;; esac
  elif [ "$a" = "--" ]; then after=1
  elif [ "$prev" = "-p" ]; then pkg="$a"
  elif [ "$prev" = "--test" ]; then :
  else case "$a" in -*) ;; *) filter="$a" ;; esac
  fi
  prev="$a"
done
if [ "$list" = 1 ]; then
  printf 'LIST %s %s exact=%s\n' "$pkg" "$filter" "$exact" >> "$state/cargo.log"
  n=0
  if [ -f "$state/inventory/$pkg" ]; then
    while IFS= read -r name; do
      [ -n "$name" ] || continue
      if [ "$exact" = 1 ]; then [ "$name" = "$filter" ] || continue
      else case "$name" in *"$filter"*) ;; *) continue ;; esac
      fi
      printf '%s: test\n' "$name"; n=$((n + 1))
    done < "$state/inventory/$pkg"
  fi
  printf '\n%s tests, 0 benchmarks\n' "$n"
  return 0
fi
printf 'RUN %s %s exact=%s live=%s\n' "$pkg" "$filter" "$exact" "${{TRANSYNC_LIVE_SMOKE:-}}" >> "$state/cargo.log"
return 0"#
                ),
            );
            let gate = Gate { h };
            for package in PACKAGES {
                gate.inventory(package, &ignored_live_tests(package));
            }
            gate
        }

        fn inventory(&self, package: &str, names: &[String]) {
            let dir = self.h.path().join("inventory");
            fs::create_dir_all(&dir).unwrap();
            fs::write(dir.join(package), names.join("\n") + "\n").unwrap();
        }

        /// The gate script with dummy credentials for every leg, so a key is
        /// never read from the repository's key files, plus `env`.
        fn run(&self, surface: &str, env: &[(&str, &str)]) -> Output {
            let mut cmd = self
                .h
                .script(&repo_root().join("scripts/smoke-live-gate.sh"));
            cmd.arg(surface)
                .env("OPENAI_API_KEY", "sk-dummy-never-sent")
                .env("ANTHROPIC_API_KEY", "dummy-never-sent")
                .env("GOOGLE_CLOUD_PROJECT", "dummy-project")
                .env("GEMINI_API_KEY", "dummy-never-sent")
                .env("GOOGLE_API_KEY", "dummy-never-sent");
            for (key, value) in env {
                cmd.env(key, value);
            }
            cmd.output().expect("smoke-live-gate.sh runs")
        }

        fn lines(&self, kind: &str) -> Vec<String> {
            self.h
                .log("cargo.log")
                .lines()
                .filter(|l| l.starts_with(kind))
                .map(str::to_string)
                .collect()
        }
    }

    /// Every ignored live test is some leg's, and every leg's test exists and
    /// is ignored in its crate — the oracle the inventory is built from.
    #[test]
    fn every_ignored_live_test_belongs_to_exactly_one_leg() {
        let mut declared: Vec<String> = PACKAGES
            .iter()
            .flat_map(|p| ignored_live_tests(p))
            .collect();
        declared.sort();
        let mut legs: Vec<String> = LEGS.iter().map(|(_, _, t)| t.to_string()).collect();
        legs.sort();
        assert_eq!(declared, legs);
    }

    #[test]
    fn each_leg_runs_its_own_test_by_exact_name() {
        for (surface, package, test) in LEGS {
            let gate = Gate::new("transync-live-leg");
            let out = gate.run(surface, &[]);
            assert_eq!(
                out.status.code(),
                Some(0),
                "{surface}: {}",
                transcript(&out)
            );
            assert_eq!(
                gate.lines("LIST"),
                vec![format!("LIST {package} {test} exact=1")],
                "{surface}"
            );
            assert_eq!(
                gate.lines("RUN"),
                vec![format!("RUN {package} {test} exact=1 live=1")],
                "{surface}"
            );
        }
    }

    /// `all` is the six legs by name, and every inventory is listed before the
    /// first paid run.
    #[test]
    fn all_lists_every_leg_before_running_any() {
        let gate = Gate::new("transync-live-all");
        let out = gate.run("all", &[]);
        assert_eq!(out.status.code(), Some(0), "{}", transcript(&out));
        let log = gate.h.log("cargo.log");
        let kinds: Vec<&str> = log.lines().map(|l| &l[..l.find(' ').unwrap()]).collect();
        assert_eq!(
            kinds,
            [vec!["LIST"; LEGS.len()], vec!["RUN"; LEGS.len()]].concat(),
            "{log}"
        );
        let mut ran = gate.lines("RUN");
        ran.sort();
        let mut expected: Vec<String> = LEGS
            .iter()
            .map(|(_, p, t)| format!("RUN {p} {t} exact=1 live=1"))
            .collect();
        expected.sort();
        assert_eq!(ran, expected);
    }

    fn assert_refused_before_spending(gate: &Gate, out: &Output, naming: &str) {
        assert_eq!(out.status.code(), Some(1), "{}", transcript(out));
        assert!(
            String::from_utf8_lossy(&out.stderr).contains(naming),
            "the refusal names {naming}:\n{}",
            transcript(out)
        );
        assert!(
            gate.lines("RUN").is_empty(),
            "nothing was spent:\n{}",
            gate.h.log("cargo.log")
        );
    }

    /// The `all` hole: with one OpenAI test gone, an empty filter still listed
    /// the other, so the leg passed having certified one surface of two.
    #[test]
    fn all_refuses_a_missing_test_before_any_paid_run() {
        let gate = Gate::new("transync-live-missing");
        gate.inventory(
            "transync-openai",
            &["live_chat_surface_round_trip".to_string()],
        );
        let out = gate.run("all", &[]);
        assert_refused_before_spending(&gate, &out, "live_responses_surface_round_trip");
    }

    /// A missing test in the LAST package is found before the first spends.
    #[test]
    fn all_refuses_a_missing_google_test_before_openai_spends() {
        let gate = Gate::new("transync-live-order");
        gate.inventory(
            "transync-google",
            &[
                "vertex_live_smoke".to_string(),
                "vertex_express_live_smoke".to_string(),
            ],
        );
        let out = gate.run("all", &[]);
        assert_refused_before_spending(&gate, &out, "developer_live_smoke");
    }

    /// A renamed test no longer stands in for the one a leg certifies.
    #[test]
    fn a_renamed_test_refuses_its_leg() {
        let gate = Gate::new("transync-live-renamed");
        gate.inventory(
            "transync-anthropic",
            &["live_messages_api_round_trip_v2".to_string()],
        );
        let out = gate.run("anthropic", &[]);
        assert_refused_before_spending(&gate, &out, "live_messages_api_round_trip");
    }

    /// R0012-0027: a malformed override for a leg this run does not call
    /// refuses nothing.
    #[test]
    fn an_override_for_an_unselected_leg_is_not_checked() {
        for (surface, key, value) in [
            ("chat", "TRANSYNC_ANTHROPIC_BASE_URL", "not-a-url"),
            ("chat", "TRANSYNC_LIVE_SMOKE_GOOGLE_MODEL", " padded"),
            ("chat", "TRANSYNC_LIVE_SMOKE_RESPONSES_MODEL", " padded"),
            ("responses", "TRANSYNC_LIVE_SMOKE_CHAT_MODEL", " padded"),
            ("anthropic", "TRANSYNC_OPENAI_BASE_URL", "not-a-url"),
            (
                "google-vertex-express",
                "TRANSYNC_OPENAI_BASE_URL",
                "not-a-url",
            ),
            (
                "google-developer",
                "TRANSYNC_LIVE_SMOKE_ANTHROPIC_MODEL",
                " padded",
            ),
        ] {
            let gate = Gate::new("transync-live-unselected");
            let out = gate.run(surface, &[(key, value)]);
            assert_eq!(
                out.status.code(),
                Some(0),
                "{surface} with {key}={value:?}: {}",
                transcript(&out)
            );
            assert_eq!(gate.lines("RUN").len(), 1, "{surface}");
        }
    }

    /// The negative controls: a selected leg still refuses its own malformed
    /// override, by name, before cargo is invoked at all — and `all` reads
    /// every one of them.
    #[test]
    fn an_override_a_selected_leg_reads_is_still_refused_before_cargo() {
        let cases = [
            ("chat", "TRANSYNC_LIVE_SMOKE_CHAT_MODEL", " m"),
            ("chat", "TRANSYNC_OPENAI_BASE_URL", "x"),
            ("responses", "TRANSYNC_LIVE_SMOKE_RESPONSES_MODEL", " m"),
            ("responses", "TRANSYNC_OPENAI_BASE_URL", "x"),
            ("anthropic", "TRANSYNC_LIVE_SMOKE_ANTHROPIC_MODEL", " m"),
            ("anthropic", "TRANSYNC_ANTHROPIC_BASE_URL", "x"),
            ("google-vertex", "TRANSYNC_LIVE_SMOKE_GOOGLE_MODEL", " m"),
            (
                "google-vertex-express",
                "TRANSYNC_LIVE_SMOKE_GOOGLE_MODEL",
                " m",
            ),
            ("google-developer", "TRANSYNC_LIVE_SMOKE_GOOGLE_MODEL", " m"),
        ];
        let all = cases.iter().map(|(_, key, value)| ("all", *key, *value));
        for (surface, key, value) in cases.iter().copied().chain(all) {
            let gate = Gate::new("transync-live-selected");
            let out = gate.run(surface, &[(key, value)]);
            assert_eq!(
                out.status.code(),
                Some(1),
                "{surface} with {key}={value:?}: {}",
                transcript(&out)
            );
            assert!(
                String::from_utf8_lossy(&out.stderr).contains(key),
                "{surface}: the refusal names {key}:\n{}",
                transcript(&out)
            );
            assert!(
                gate.h.log("cargo.log").is_empty(),
                "{surface}: refused before cargo:\n{}",
                gate.h.log("cargo.log")
            );
        }
    }
}
