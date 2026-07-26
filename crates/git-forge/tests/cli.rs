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
fn issue_put_get_list_log_and_remove() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    let path = dir.path();

    let (_, err, ok) = run(
        path,
        &[
            "issue",
            "put",
            "issue-1",
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
    assert!(ok, "issue put failed: {err}");

    let (out, err, ok) = run(path, &["issue", "get", "issue-1"]);
    assert!(ok, "issue get failed: {err}");
    assert!(out.contains("id: issue-1"), "issue get output: {out}");
    assert!(out.contains("body: first body"), "issue get output: {out}");
    assert!(out.contains("labels: bug,p1"), "issue get output: {out}");
    assert!(out.contains("assignees: alice"), "issue get output: {out}");
    assert!(out.contains("reporters: bob"), "issue get output: {out}");

    let (out, err, ok) = run(path, &["issue", "ls"]);
    assert!(ok, "issue ls failed: {err}");
    assert_eq!(out.trim(), "issue-1");

    let (_, err, ok) = run(
        path,
        &[
            "issue",
            "put",
            "issue-1",
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
    assert!(ok, "second issue put failed: {err}");

    let (out, err, ok) = run(path, &["issue", "log", "issue-1"]);
    assert!(ok, "issue log failed: {err}");
    let lines: Vec<&str> = out.lines().collect();
    assert!(lines.len() >= 2, "issue log output: {out}");

    let (_, err, ok) = run(path, &["issue", "rm", "issue-1"]);
    assert!(ok, "issue rm failed: {err}");

    let (_, _, ok) = run(path, &["issue", "get", "issue-1"]);
    assert!(!ok, "issue get after rm should fail");
}

#[test]
fn review_put_get_list_log_and_remove() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    let path = dir.path();

    let (_, err, ok) = run(
        path,
        &[
            "review",
            "put",
            "review-1",
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
    assert!(ok, "review put failed: {err}");

    let (out, err, ok) = run(path, &["review", "get", "review-1"]);
    assert!(ok, "review get failed: {err}");
    assert!(out.contains("id: review-1"), "review get output: {out}");
    assert!(out.contains("body: looks good"), "review get output: {out}");
    assert!(out.contains("reviewers: carol"), "review get output: {out}");
    assert!(out.contains("requesters: dave"), "review get output: {out}");
    assert!(
        out.contains("target: commit:deadbeef"),
        "review get output: {out}"
    );

    let (out, err, ok) = run(path, &["review", "ls"]);
    assert!(ok, "review ls failed: {err}");
    assert_eq!(out.trim(), "review-1");

    let (_, err, ok) = run(
        path,
        &[
            "review",
            "put",
            "review-1",
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
    assert!(ok, "second review put failed: {err}");

    let (out, err, ok) = run(path, &["review", "log", "review-1"]);
    assert!(ok, "review log failed: {err}");
    let lines: Vec<&str> = out.lines().collect();
    assert!(lines.len() >= 2, "review log output: {out}");

    let (_, err, ok) = run(path, &["review", "rm", "review-1"]);
    assert!(ok, "review rm failed: {err}");

    let (_, _, ok) = run(path, &["review", "get", "review-1"]);
    assert!(!ok, "review get after rm should fail");
}
