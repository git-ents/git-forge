//! Shared scaffolding for the workspace's integration tests.

use std::io::Write;
use std::path::Path;

/// Initialize a git repository at `path` with a committer identity configured,
/// so `gix::Repository::commit` has an author and committer to record.
pub fn init_repo(path: &Path) {
    let repo = gix::init(path).expect("init repo");
    let mut config = std::fs::OpenOptions::new()
        .append(true)
        .open(repo.git_dir().join("config"))
        .expect("open config");
    writeln!(config, "[user]\n\tname = Test\n\temail = test@example.com").expect("write config");
}

/// Initialize a repository at `path` (if not already one), write `rel` with
/// `contents`, and commit it -- for tests that need a real, committed blob to
/// anchor against (`gix_anchor::capture` resolves a path against a commit's
/// tree, which an empty repository has none of).
///
/// Shells out to `git` rather than writing objects through `gix`, so the
/// commit a test anchors against is built the same way a real user's would
/// be, not by the crate under test's own writer.
pub fn commit_file(path: &Path, rel: &str, contents: &str, message: &str) {
    if gix::open(path).is_err() {
        init_repo(path);
    }
    let full = path.join(rel);
    if let Some(parent) = full.parent() {
        std::fs::create_dir_all(parent).expect("create parent");
    }
    std::fs::write(&full, contents).expect("write fixture file");
    run_git(path, &["add", "--", rel]);
    run_git(path, &["commit", "--quiet", "-m", message]);
}

fn run_git(path: &Path, args: &[&str]) {
    let status = std::process::Command::new("git")
        .arg("-C")
        .arg(path)
        .args(args)
        .env("GIT_AUTHOR_NAME", "Test")
        .env("GIT_AUTHOR_EMAIL", "test@example.com")
        .env("GIT_COMMITTER_NAME", "Test")
        .env("GIT_COMMITTER_EMAIL", "test@example.com")
        .status()
        .unwrap_or_else(|e| panic!("run git {args:?}: {e}"));
    assert!(status.success(), "git {args:?} failed");
}
