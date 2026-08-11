//! `git forge ui` launches a separate `git-forge-ui` executable, so the argv
//! it builds and the rules it uses to find that executable are a contract
//! between two independently built binaries. Nothing else in the tree checks
//! it: the UI's own tests drive the server directly and never see the CLI.
//!
//! Every test here installs a stub `git-forge-ui` that prints its argv and
//! exits, so the CLI's own launch path is what is under test.

// The stubs are shell scripts and the launch path under test is `exec`.
#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use test_support::init_repo;

const BIN: &str = env!("CARGO_BIN_EXE_git-forge");

/// A directory holding a copy of the CLI, so that `current_exe()` resolves
/// beside stubs we control rather than beside the real build output.
struct Install {
    dir: tempfile::TempDir,
}

impl Install {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        std::fs::copy(BIN, dir.path().join("git-forge")).unwrap();
        Self { dir }
    }

    fn cli(&self) -> PathBuf {
        self.dir.path().join("git-forge")
    }

    /// Write a `git-forge-ui` that echoes its arguments instead of serving.
    fn stub_ui(&self, dir: &Path, marker: &str) -> PathBuf {
        write_stub(&dir.join("git-forge-ui"), marker)
    }
}

fn write_stub(path: &Path, marker: &str) -> PathBuf {
    std::fs::write(path, format!("#!/bin/sh\necho \"{marker} $@\"\n")).unwrap();
    make_executable(path);
    path.to_owned()
}

fn make_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = std::fs::metadata(path).unwrap().permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(path, permissions).unwrap();
}

/// The value the stub received for `flag`, from its echoed argv.
fn argument_after(output: &str, flag: &str) -> Option<String> {
    let mut words = output.split_whitespace();
    words.find(|word| *word == flag)?;
    words.next().map(str::to_owned)
}

/// Run `git forge ui` in `repo` with an explicit `PATH`, returning merged output.
fn run_ui(cli: &Path, repo: &Path, path_var: &str) -> String {
    let out = Command::new(cli)
        .current_dir(repo)
        .args(["ui", "--host", "127.0.0.1", "--port", "7777"])
        .env("PATH", path_var)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap();
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

#[test]
fn launches_the_sibling_executable_with_the_resolved_git_dir() {
    let install = Install::new();
    let repo = tempfile::tempdir().unwrap();
    init_repo(repo.path());
    install.stub_ui(install.dir.path(), "SIBLING");

    let output = run_ui(&install.cli(), repo.path(), "/usr/bin:/bin");

    assert!(
        output.starts_with("SIBLING "),
        "unexpected launch: {output}"
    );
    // The CLI must hand over the Git directory it already resolved rather than
    // let the UI rediscover one, or `core.worktree` sends them to different
    // repositories.
    let git_dir = argument_after(&output, "--git-dir").expect("no --git-dir");
    // A detached server outlives the shell that started it, so a working
    // directory relative path would not survive.
    assert!(
        Path::new(&git_dir).is_absolute(),
        "--git-dir is relative: {git_dir}"
    );
    assert_eq!(
        std::fs::canonicalize(&git_dir).unwrap(),
        std::fs::canonicalize(repo.path().join(".git")).unwrap(),
        "wrong repository: {output}"
    );
    assert!(output.contains("--host 127.0.0.1"), "no host: {output}");
    assert!(output.contains("--port 7777"), "no port: {output}");
    // `--open` is off by default and must not be forwarded.
    assert!(!output.contains("--open"), "unexpected --open: {output}");
}

#[test]
fn forwards_open_only_when_asked() {
    let install = Install::new();
    let repo = tempfile::tempdir().unwrap();
    init_repo(repo.path());
    install.stub_ui(install.dir.path(), "SIBLING");

    let out = Command::new(install.cli())
        .current_dir(repo.path())
        .args(["ui", "--host", "127.0.0.1", "--port", "7777", "--open"])
        .env("PATH", "/usr/bin:/bin")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let output = String::from_utf8_lossy(&out.stdout).into_owned();

    assert!(output.contains("--open"), "--open was dropped: {output}");
}

#[test]
fn an_empty_path_entry_never_runs_an_executable_from_the_repository() {
    let install = Install::new();
    let repo = tempfile::tempdir().unwrap();
    init_repo(repo.path());
    // A repository is untrusted content: a clone may well carry a file named
    // `git-forge-ui`. An empty PATH entry means "the current directory" to a
    // shell, and must not mean that here.
    write_stub(&repo.path().join("git-forge-ui"), "FROM-REPOSITORY");
    std::fs::remove_file(install.dir.path().join("git-forge-ui")).ok();

    for path_var in [":/usr/bin:/bin", "/usr/bin:/bin:", "/usr/bin::/bin"] {
        let output = run_ui(&install.cli(), repo.path(), path_var);
        assert!(
            !output.contains("FROM-REPOSITORY"),
            "PATH {path_var:?} ran a binary from the repository: {output}"
        );
        assert!(
            output.contains("cannot find `git-forge-ui`"),
            "PATH {path_var:?} gave no actionable error: {output}"
        );
    }
}

#[test]
fn a_non_executable_candidate_does_not_end_the_search() {
    let install = Install::new();
    let repo = tempfile::tempdir().unwrap();
    init_repo(repo.path());
    // A readable-but-not-executable sibling — a partial download, a stray
    // completion script — must fall through to the copy on PATH.
    std::fs::write(install.dir.path().join("git-forge-ui"), "not a program").unwrap();
    let on_path = tempfile::tempdir().unwrap();
    install.stub_ui(on_path.path(), "FROM-PATH");

    let output = run_ui(
        &install.cli(),
        repo.path(),
        &format!("{}:/usr/bin:/bin", on_path.path().display()),
    );

    assert!(
        output.starts_with("FROM-PATH "),
        "unexpected launch: {output}"
    );
}

#[test]
fn a_missing_executable_names_where_it_looked() {
    let install = Install::new();
    let repo = tempfile::tempdir().unwrap();
    init_repo(repo.path());

    let output = run_ui(&install.cli(), repo.path(), "/usr/bin:/bin");

    assert!(
        output.contains("cannot find `git-forge-ui`"),
        "unhelpful error: {output}"
    );
    assert!(
        output.contains(install.dir.path().to_str().unwrap()),
        "error does not say where it looked: {output}"
    );
}
