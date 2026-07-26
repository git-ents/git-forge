//! Drive the built `git-forge` binary against a temp repo, exactly as
//! `git forge …` would.

use std::path::Path;
use std::process::{Command, Stdio};

use test_support::init_repo;

const BIN: &str = env!("CARGO_BIN_EXE_git-forge");

/// Run the binary in `dir`, returning `(stdout, stderr, ok)`.
fn run(dir: &Path, args: &[&str]) -> (String, String, bool) {
    let out = Command::new(BIN)
        .current_dir(dir)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap();

    (
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
        out.status.success(),
    )
}

fn git_stdout(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .current_dir(dir)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

fn create_origin_commit(dir: &Path) -> String {
    let _ = git_stdout(dir, &["commit", "--allow-empty", "-m", "init"]);
    git_stdout(dir, &["rev-parse", "--short=8", "HEAD"])
}

#[test]
fn bare_cli_lists_groups() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());

    let (_, err, ok) = run(dir.path(), &[]);
    assert!(!ok, "bare command should fail without subcommand");
    assert!(
        err.contains("Usage: git-forge <COMMAND>"),
        "bare command stderr: {err}"
    );
}

#[test]
fn install_publishes_forge_schemas() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());

    let (out, err, ok) = run(dir.path(), &["install"]);
    assert!(ok, "install failed: {err}");
    assert!(out.contains("issue "), "install output: {out}");
    assert!(out.contains("review "), "install output: {out}");
}

#[test]
fn issue_new_get_list_log_and_remove() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    let path = dir.path();
    let issue_id = create_origin_commit(path);

    let (_, err, ok) = run(
        path,
        &[
            "issue",
            "new",
            "--body",
            "first body",
            "--label",
            "bug",
            "--label",
            "p1",
            "--assignee",
            "alice",
            "--reporter",
            "bob",
        ],
    );
    assert!(ok, "issue new failed: {err}");

    let get_args = vec!["issue", "get", issue_id.as_str()];
    let (out, err, ok) = run(path, &get_args);
    assert!(ok, "issue get failed: {err}");
    assert!(
        out.contains(&format!("id: {issue_id}")),
        "issue get output: {out}"
    );
    assert!(out.contains("body: first body"), "issue get output: {out}");
    assert!(out.contains("labels: bug,p1"), "issue get output: {out}");
    assert!(out.contains("assignees: alice"), "issue get output: {out}");
    assert!(out.contains("reporters: bob"), "issue get output: {out}");

    let (out, err, ok) = run(path, &["issue", "ls"]);
    assert!(ok, "issue ls failed: {err}");
    assert_eq!(out.trim(), issue_id);

    let (_, err, ok) = run(
        path,
        &[
            "issue",
            "new",
            "--body",
            "second body",
            "--label",
            "bug",
            "--assignee",
            "alice",
            "--reporter",
            "bob",
        ],
    );
    assert!(ok, "second issue new failed: {err}");

    let log_args = vec!["issue", "log", issue_id.as_str()];
    let (out, err, ok) = run(path, &log_args);
    assert!(ok, "issue log failed: {err}");
    let lines: Vec<&str> = out.lines().collect();
    assert!(lines.len() >= 2, "issue log output: {out}");

    let rm_args = vec!["issue", "rm", issue_id.as_str()];
    let (_, err, ok) = run(path, &rm_args);
    assert!(ok, "issue rm failed: {err}");

    let (_, _, ok) = run(path, &get_args);
    assert!(!ok, "issue get after rm should fail");
}

#[test]
fn issue_new_interactive_requires_terminal() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());

    let (_, err, ok) = run(dir.path(), &["issue", "new", "-i"]);
    assert!(!ok, "interactive issue new should fail without terminal");
    assert!(
        err.contains("--interactive requires a terminal"),
        "interactive issue put stderr: {err}"
    );
}

#[test]
fn review_new_interactive_requires_terminal() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());

    let (_, err, ok) = run(dir.path(), &["review", "new", "-i"]);
    assert!(!ok, "interactive review new should fail without terminal");
    assert!(
        err.contains("--interactive requires a terminal"),
        "interactive review put stderr: {err}"
    );
}

#[test]
fn review_new_get_list_log_and_remove() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    let path = dir.path();
    let review_id = create_origin_commit(path);

    let (_, err, ok) = run(
        path,
        &[
            "review",
            "new",
            "--body",
            "looks good",
            "--reviewer",
            "carol",
            "--requester",
            "dave",
            "--target",
            "commit:deadbeef",
        ],
    );
    assert!(ok, "review new failed: {err}");

    let get_args = vec!["review", "get", review_id.as_str()];
    let (out, err, ok) = run(path, &get_args);
    assert!(ok, "review get failed: {err}");
    assert!(
        out.contains(&format!("id: {review_id}")),
        "review get output: {out}"
    );
    assert!(out.contains("body: looks good"), "review get output: {out}");
    assert!(out.contains("reviewers: carol"), "review get output: {out}");
    assert!(out.contains("requesters: dave"), "review get output: {out}");
    assert!(
        out.contains("target: commit:deadbeef"),
        "review get output: {out}"
    );

    let (out, err, ok) = run(path, &["review", "ls"]);
    assert!(ok, "review ls failed: {err}");
    assert_eq!(out.trim(), review_id);

    let (_, err, ok) = run(
        path,
        &[
            "review",
            "new",
            "--body",
            "needs changes",
            "--reviewer",
            "carol",
            "--requester",
            "dave",
            "--target",
            "commit:feedface",
        ],
    );
    assert!(ok, "second review new failed: {err}");

    let log_args = vec!["review", "log", review_id.as_str()];
    let (out, err, ok) = run(path, &log_args);
    assert!(ok, "review log failed: {err}");
    let lines: Vec<&str> = out.lines().collect();
    assert!(lines.len() >= 2, "review log output: {out}");

    let rm_args = vec!["review", "rm", review_id.as_str()];
    let (_, err, ok) = run(path, &rm_args);
    assert!(ok, "review rm failed: {err}");

    let (_, _, ok) = run(path, &get_args);
    assert!(!ok, "review get after rm should fail");
}
