//! Drive the built `git-forge` binary against a temp repo, exactly as
//! `git forge …` would.

use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use gix_forge::{Issue, Review, ReviewTarget};
use proptest::prelude::*;
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

/// Run the binary attached to a pseudo-terminal via `script`. Each input item
/// waits until `wait_for` has appeared in the child's output before writing
/// `text` to stdin, avoiding races with the pty setup. Returns `(stdout,
/// stderr, ok)` of the `script` process.
fn run_with_pty_env(
    dir: &Path,
    args: &[&str],
    inputs: &[(&str, &str)],
    envs: &[(&str, &str)],
) -> (String, String, bool) {
    let mut cmd = Command::new("script");
    cmd.current_dir(dir)
        .arg("-q")
        .arg("/dev/null")
        .arg(BIN)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    for (k, v) in envs {
        cmd.env(k, v);
    }

    let mut child = cmd.spawn().unwrap();
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();

    let stdout_buf = Arc::new(Mutex::new(Vec::new()));
    let stdout_reader = {
        let stdout_buf = Arc::clone(&stdout_buf);
        std::thread::spawn(move || {
            let mut byte = [0u8; 1];
            while let Ok(1) = stdout.read(&mut byte) {
                stdout_buf.lock().unwrap().push(byte[0]);
            }
        })
    };

    // Keep the write end of stdin open until the child has exited: `script`
    // translates a closed pipe into a literal EOF byte written into the pty,
    // which can race with (and get echoed ahead of) data we just wrote.
    let mut stdin_handle = child.stdin.take();

    for (wait_for, text) in inputs {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let seen = String::from_utf8_lossy(&stdout_buf.lock().unwrap()).contains(wait_for);
            if seen || Instant::now() >= deadline {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        stdin_handle
            .as_mut()
            .unwrap()
            .write_all(text.as_bytes())
            .unwrap();
    }

    let mut stderr_buf = Vec::new();
    stderr.read_to_end(&mut stderr_buf).unwrap();

    let status = child.wait().unwrap();
    stdout_reader.join().unwrap();

    drop(stdin_handle);

    (
        String::from_utf8_lossy(&stdout_buf.lock().unwrap()).into_owned(),
        String::from_utf8_lossy(&stderr_buf).into_owned(),
        status.success(),
    )
}

fn bulleted_items(out: &str) -> Vec<String> {
    out.lines()
        .filter_map(|line| line.strip_prefix("  - "))
        .map(ToOwned::to_owned)
        .collect()
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

fn create_origin_commit(dir: &Path) {
    let _ = git_stdout(dir, &["commit", "--allow-empty", "-m", "init"]);
}

fn created_id(out: &str) -> String {
    out.lines().next().unwrap_or_default().trim().to_owned()
}

fn put_issue(dir: &Path, id: &str, body: &str) {
    let repo = gix::open(dir).unwrap();
    let issue = Issue {
        id: id.to_owned(),
        status: "open".to_owned(),
        title: String::new(),
        body: body.to_owned(),
        labels: vec![],
        assignees: vec![],
        reporters: vec![],
        edit: None,
    };
    issue.save_in_repo(&repo).unwrap();
}

fn put_review(dir: &Path, id: &str, body: &str) {
    let repo = gix::open(dir).unwrap();
    let review = Review {
        id: id.to_owned(),
        status: "open".to_owned(),
        body: body.to_owned(),
        reviewers: vec![],
        requesters: vec![],
        target: ReviewTarget::Commit {
            oid: "deadbeef".to_owned(),
        },
        edit: None,
    };
    review.save_in_repo(&repo).unwrap();
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
fn issue_new_show_list_log_and_remove() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    let path = dir.path();
    create_origin_commit(path);

    let (out, err, ok) = run(
        path,
        &[
            "issue",
            "new",
            "--title",
            "first title",
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
    let issue_id = created_id(&out);

    let show_args = vec!["issue", "show", issue_id.as_str()];
    let (out, err, ok) = run(path, &show_args);
    assert!(ok, "issue show failed: {err}");
    assert!(out.contains("first title"), "issue show output: {out}");
    assert!(out.contains("first body"), "issue show output: {out}");
    assert!(out.contains("bug, p1"), "issue show output: {out}");
    assert!(out.contains("alice"), "issue show output: {out}");
    assert!(out.contains("bob"), "issue show output: {out}");

    let (out, err, ok) = run(path, &["issue", "ls"]);
    assert!(ok, "issue ls failed: {err}");
    assert!(out.contains("ID"), "issue ls output: {out}");
    assert!(out.contains("TITLE"), "issue ls output: {out}");
    assert!(
        out.contains(&format!("#{issue_id}")),
        "issue ls output: {out}"
    );

    let (_, err, ok) = run(
        path,
        &["issue", "edit", issue_id.as_str(), "--body", "second body"],
    );
    assert!(ok, "issue edit failed: {err}");

    let (out, err, ok) = run(path, &show_args);
    assert!(ok, "issue show after edit failed: {err}");
    assert!(out.contains("second body"), "issue show output: {out}");

    let log_args = vec!["issue", "log", issue_id.as_str()];
    let (out, err, ok) = run(path, &log_args);
    assert!(ok, "issue log failed: {err}");
    assert!(
        out.contains(&format!("issue history {issue_id}:")),
        "issue log output: {out}"
    );
    let lines: Vec<&str> = out.lines().collect();
    assert!(lines.len() >= 3, "issue log output: {out}");

    let rm_args = vec!["issue", "rm", issue_id.as_str()];
    let (_, err, ok) = run(path, &rm_args);
    assert!(ok, "issue rm failed: {err}");

    let (_, _, ok) = run(path, &show_args);
    assert!(!ok, "issue show after rm should fail");
}

#[test]
fn issue_show_accepts_min_unique_prefix_and_renders_ambiguous_matches() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    let path = dir.path();

    put_issue(path, "abc11111", "first body");
    put_issue(path, "abc22222", "second body");

    let (out, err, ok) = run(path, &["issue", "show", "abc1"]);
    assert!(ok, "issue show by unique prefix failed: {err}");
    assert!(out.contains("abc11111"), "issue show output: {out}");
    assert!(out.contains("first body"), "issue show output: {out}");
    assert!(!out.contains("second body"), "issue show output: {out}");

    let (_, err, ok) = run(path, &["issue", "show", "abc"]);
    assert!(!ok, "issue show should fail on ambiguous prefix");
    assert!(
        err.contains("ambiguous issue id abc"),
        "issue show stderr: {err}"
    );
    assert!(
        err.contains("**abc1**1111") && err.contains("**abc2**2222"),
        "issue show stderr: {err}"
    );
}

#[test]
fn issue_new_without_args_requires_body_without_terminal() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());

    let (_, err, ok) = run(dir.path(), &["issue", "new"]);
    assert!(!ok, "issue new should fail without args and terminal");
    assert!(
        err.contains("--body is required unless running interactively"),
        "issue new stderr: {err}"
    );
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
fn issue_edit_without_args_requires_field_without_terminal() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    let path = dir.path();
    create_origin_commit(path);

    let (out, err, ok) = run(path, &["issue", "new", "--body", "first body"]);
    assert!(ok, "issue new failed: {err}");
    let issue_id = created_id(&out);

    let (_, err, ok) = run(path, &["issue", "edit", issue_id.as_str()]);
    assert!(!ok, "issue edit should fail without args and terminal");
    assert!(
        err.contains("--title, --body, or --status is required unless running interactively"),
        "issue edit stderr: {err}"
    );
}

#[test]
fn issue_edit_picker_only_edits_selected_field_with_pty() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    let path = dir.path();
    create_origin_commit(path);

    let (out, err, ok) = run(
        path,
        &[
            "issue",
            "new",
            "--title",
            "first title",
            "--body",
            "first body",
        ],
    );
    assert!(ok, "issue new failed: {err}");
    let issue_id = created_id(&out);

    let editor_script = path.join("editor-write-issue.sh");
    std::fs::write(
        &editor_script,
        "#!/bin/sh\ncat > \"$1\" <<'EOF'\nsecond body\nEOF\n",
    )
    .unwrap();

    let editor_cmd = format!("sh {}", editor_script.to_string_lossy());
    let (_out, err, ok) = run_with_pty_env(
        path,
        &["issue", "edit", issue_id.as_str()],
        &[("What would you like to edit?", "\x1b[B \r")],
        &[("EDITOR", editor_cmd.as_str())],
    );
    assert!(ok, "interactive issue edit failed: {err}");

    let (out, err, ok) = run(path, &["issue", "show", issue_id.as_str()]);
    assert!(ok, "issue show after edit failed: {err}");
    assert!(out.contains("first title"), "issue show output: {out}");
    assert!(out.contains("second body"), "issue show output: {out}");
}

#[test]
fn issue_edit_picker_prompts_selected_terminal_field_with_pty() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    let path = dir.path();
    create_origin_commit(path);

    let (out, err, ok) = run(
        path,
        &[
            "issue",
            "new",
            "--title",
            "first title",
            "--body",
            "first body",
        ],
    );
    assert!(ok, "issue new failed: {err}");
    let issue_id = created_id(&out);

    let (_out, err, ok) = run_with_pty_env(
        path,
        &["issue", "edit", issue_id.as_str()],
        &[
            ("What would you like to edit?", " \r"),
            ("Title (first title)", "second title\n"),
        ],
        &[],
    );
    assert!(ok, "interactive issue edit failed: {err}");

    let (out, err, ok) = run(path, &["issue", "show", issue_id.as_str()]);
    assert!(ok, "issue show after edit failed: {err}");
    assert!(out.contains("second title"), "issue show output: {out}");
    assert!(out.contains("first body"), "issue show output: {out}");
}

#[test]
fn review_show_accepts_min_unique_prefix_and_renders_ambiguous_matches() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    let path = dir.path();

    put_review(path, "def11111", "first review");
    put_review(path, "def22222", "second review");

    let (out, err, ok) = run(path, &["review", "show", "def1"]);
    assert!(ok, "review show by unique prefix failed: {err}");
    assert!(out.contains("review"), "review show output: {out}");
    assert!(out.contains("def11111"), "review show output: {out}");
    assert!(out.contains("Open"), "review show output: {out}");
    assert!(out.contains("first review"), "review show output: {out}");

    let (_, err, ok) = run(path, &["review", "show", "def"]);
    assert!(!ok, "review show should fail on ambiguous prefix");
    assert!(
        err.contains("ambiguous review id def"),
        "review show stderr: {err}"
    );
    assert!(
        err.contains("**def1**1111") && err.contains("**def2**2222"),
        "review show stderr: {err}"
    );
}

#[test]
fn review_new_without_args_requires_body_without_terminal() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());

    let (_, err, ok) = run(dir.path(), &["review", "new"]);
    assert!(!ok, "review new should fail without args and terminal");
    assert!(
        err.contains("--body is required unless running interactively"),
        "review new stderr: {err}"
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
fn review_edit_without_args_requires_field_without_terminal() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    let path = dir.path();
    create_origin_commit(path);

    let (out, err, ok) = run(
        path,
        &[
            "review",
            "new",
            "--body",
            "looks good",
            "--target",
            "commit:deadbeef",
        ],
    );
    assert!(ok, "review new failed: {err}");
    let review_id = created_id(&out);

    let (_, err, ok) = run(path, &["review", "edit", review_id.as_str()]);
    assert!(!ok, "review edit should fail without args and terminal");
    assert!(
        err.contains("--body, --target, or --status is required unless running interactively"),
        "review edit stderr: {err}"
    );
}

#[test]
fn review_new_show_list_log_and_remove() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    let path = dir.path();
    create_origin_commit(path);

    let (out, err, ok) = run(
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
    let review_id = created_id(&out);

    let show_args = vec!["review", "show", review_id.as_str()];
    let (out, err, ok) = run(path, &show_args);
    assert!(ok, "review show failed: {err}");
    assert!(out.contains("review"), "review show output: {out}");
    assert!(out.contains(&review_id), "review show output: {out}");
    assert!(out.contains("Open"), "review show output: {out}");
    assert!(out.contains("looks good"), "review show output: {out}");
    assert!(out.contains("carol"), "review show output: {out}");
    assert!(out.contains("dave"), "review show output: {out}");
    assert!(out.contains("commit:deadbeef"), "review show output: {out}");

    let (out, err, ok) = run(path, &["review", "ls"]);
    assert!(ok, "review ls failed: {err}");
    assert!(out.contains("ID"), "review ls output: {out}");
    assert!(out.contains("TARGET"), "review ls output: {out}");
    assert!(
        out.contains(&format!("#{review_id}")),
        "review ls output: {out}"
    );

    let (_, err, ok) = run(
        path,
        &[
            "review",
            "edit",
            review_id.as_str(),
            "--body",
            "needs changes",
            "--target",
            "commit:feedface",
        ],
    );
    assert!(ok, "review edit failed: {err}");

    let (out, err, ok) = run(path, &show_args);
    assert!(ok, "review show after edit failed: {err}");
    assert!(out.contains("needs changes"), "review show output: {out}");
    assert!(out.contains("commit:feedface"), "review show output: {out}");

    let log_args = vec!["review", "log", review_id.as_str()];
    let (out, err, ok) = run(path, &log_args);
    assert!(ok, "review log failed: {err}");
    assert!(
        out.contains(&format!("review history {review_id}:")),
        "review log output: {out}"
    );
    let lines: Vec<&str> = out.lines().collect();
    assert!(lines.len() >= 3, "review log output: {out}");

    let rm_args = vec!["review", "rm", review_id.as_str()];
    let (_, err, ok) = run(path, &rm_args);
    assert!(ok, "review rm failed: {err}");

    let (_, _, ok) = run(path, &show_args);
    assert!(!ok, "review show after rm should fail");
}

#[test]
fn review_edit_picker_only_edits_selected_field_with_pty() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    let path = dir.path();
    create_origin_commit(path);

    let (out, err, ok) = run(
        path,
        &[
            "review",
            "new",
            "--body",
            "looks good",
            "--target",
            "commit:deadbeef",
        ],
    );
    assert!(ok, "review new failed: {err}");
    let review_id = created_id(&out);

    let editor_script = path.join("editor-write-review.sh");
    std::fs::write(
        &editor_script,
        "#!/bin/sh\ncat > \"$1\" <<'EOF'\nneeds changes\nEOF\n",
    )
    .unwrap();

    let editor_cmd = format!("sh {}", editor_script.to_string_lossy());
    let (_out, err, ok) = run_with_pty_env(
        path,
        &["review", "edit", review_id.as_str()],
        &[("What would you like to edit?", " \r")],
        &[("EDITOR", editor_cmd.as_str())],
    );
    assert!(ok, "interactive review edit failed: {err}");

    let (out, err, ok) = run(path, &["review", "show", review_id.as_str()]);
    assert!(ok, "review show after edit failed: {err}");
    assert!(out.contains("needs changes"), "review show output: {out}");
    assert!(out.contains("deadbeef"), "review show output: {out}");
}

#[test]
fn comment_edit_without_args_requires_edit_without_terminal() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());

    let (_, err, ok) = run(dir.path(), &["comment", "edit", "comment-1"]);
    assert!(!ok, "comment edit should fail without args and terminal");
    assert!(
        err.contains("--edit is required unless running interactively"),
        "comment edit stderr: {err}"
    );
}

#[test]
fn comment_edit_interactive_requires_terminal() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());

    let (_, err, ok) = run(dir.path(), &["comment", "edit", "comment-1", "-i"]);
    assert!(!ok, "interactive comment edit should fail without terminal");
    assert!(
        err.contains("--interactive requires a terminal"),
        "interactive comment edit stderr: {err}"
    );
}

#[test]
fn comment_edit_without_edit_defaults_to_interactive_with_pty() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    let path = dir.path();

    let (_out, err, ok) = run_with_pty_env(
        path,
        &["comment", "edit", "comment-1"],
        &[("Edit reason", "editor reason\n")],
        &[],
    );
    assert!(ok, "interactive comment edit failed: {err}");

    let (out, err, ok) = run(path, &["comment", "log", "comment-1"]);
    assert!(ok, "comment log failed: {err}");
    assert!(
        out.contains("comment history comment-1:"),
        "comment log output: {out}"
    );
}

#[test]
fn comment_edit_and_log() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    let path = dir.path();

    let comment_id = "comment-1";
    let (_, err, ok) = run(
        path,
        &["comment", "edit", comment_id, "--edit", "fix wording"],
    );
    assert!(ok, "comment edit failed: {err}");

    let (out, err, ok) = run(path, &["comment", "log", comment_id]);
    assert!(ok, "comment log failed: {err}");
    assert!(
        out.contains(&format!("comment history {comment_id}:")),
        "comment log output: {out}"
    );
    let lines: Vec<&str> = out.lines().collect();
    assert!(lines.len() >= 2, "comment log output: {out}");
}

#[test]
fn query_sugar_filters_by_people_and_keyword() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    let path = dir.path();

    create_origin_commit(path);
    let (out, err, ok) = run(
        path,
        &[
            "issue",
            "new",
            "--body",
            "release blocker",
            "--assignee",
            "alice",
            "--reporter",
            "bob",
        ],
    );
    assert!(ok, "issue new failed: {err}");
    let issue_id = created_id(&out);

    create_origin_commit(path);
    let (out, err, ok) = run(
        path,
        &[
            "review",
            "new",
            "--body",
            "release reviewed",
            "--reviewer",
            "carol",
            "--requester",
            "dave",
            "--target",
            "commit:deadbeef",
        ],
    );
    assert!(ok, "review new failed: {err}");
    let review_id = created_id(&out);

    let (out, err, ok) = run(path, &["query", "assignee", "alice"]);
    assert!(ok, "query assignee failed: {err}");
    assert!(
        out.contains("issues assigned to alice:"),
        "query assignee output: {out}"
    );
    assert_eq!(bulleted_items(&out), vec![issue_id.clone()]);

    let (out, err, ok) = run(path, &["query", "reviewer", "carol"]);
    assert!(ok, "query reviewer failed: {err}");
    assert!(
        out.contains("reviews by reviewer carol:"),
        "query reviewer output: {out}"
    );
    assert_eq!(bulleted_items(&out), vec![review_id.clone()]);

    let (out, err, ok) = run(path, &["query", "requester", "dave"]);
    assert!(ok, "query requester failed: {err}");
    assert!(
        out.contains("reviews by requester dave:"),
        "query requester output: {out}"
    );
    assert_eq!(bulleted_items(&out), vec![review_id.clone()]);

    let (out, err, ok) = run(path, &["query", "keyword", "release"]);
    assert!(ok, "query keyword failed: {err}");
    assert!(
        out.contains(&format!("  - issue {issue_id}")),
        "query keyword output: {out}"
    );
    assert!(
        out.contains(&format!("  - review {review_id}")),
        "query keyword output: {out}"
    );

    let (out, err, ok) = run(path, &["query", "title", "reviewed"]);
    assert!(ok, "query title alias failed: {err}");
    assert_eq!(bulleted_items(&out), vec![format!("review {review_id}")]);

    let (out, err, ok) = run(
        path,
        &[
            "query",
            "find",
            "--assignee",
            "alice",
            "--keyword",
            "blocker",
        ],
    );
    assert!(ok, "query find issue filter failed: {err}");
    assert_eq!(bulleted_items(&out), vec![format!("issue {issue_id}")]);

    let (out, err, ok) = run(
        path,
        &[
            "query",
            "find",
            "--reviewer",
            "carol",
            "--requester",
            "dave",
            "--title",
            "reviewed",
        ],
    );
    assert!(ok, "query find review filter failed: {err}");
    assert_eq!(bulleted_items(&out), vec![format!("review {review_id}")]);

    let (_, err, ok) = run(path, &["query", "find"]);
    assert!(!ok, "query find without filters should fail");
    assert!(
        err.contains("requires at least one filter"),
        "query find without filters stderr: {err}"
    );
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(96))]

    #[test]
    fn query_find_supports_all_filter_combinations(
        use_assignee in any::<bool>(),
        use_reviewer in any::<bool>(),
        use_requester in any::<bool>(),
        text_filter_kind in 0u8..3,
    ) {
        prop_assume!(use_assignee || use_reviewer || use_requester || text_filter_kind != 0);

        let dir = tempfile::tempdir().unwrap();
        init_repo(dir.path());
        let path = dir.path();

        let repo = gix::open(path).unwrap();

        let issue_rows = vec![
            ("iss11111", "release blocker", "alice"),
            ("iss22222", "misc task", "eve"),
        ];
        for (id, body, assignee) in &issue_rows {
            Issue {
                id: (*id).to_owned(),
                status: "open".to_owned(),
                title: String::new(),
                body: (*body).to_owned(),
                labels: vec![],
                assignees: vec![(*assignee).to_owned()],
                reporters: vec![],
                edit: None,
            }
            .save_in_repo(&repo)
            .unwrap();
        }

        let review_rows = vec![
            ("rev11111", "release reviewed", "carol", "dave", "deadbeef"),
            ("rev22222", "other note", "mallory", "trent", "feedface"),
        ];
        for (id, body, reviewer, requester, oid) in &review_rows {
            Review {
                id: (*id).to_owned(),
                status: "open".to_owned(),
                body: (*body).to_owned(),
                reviewers: vec![(*reviewer).to_owned()],
                requesters: vec![(*requester).to_owned()],
                target: ReviewTarget::Commit {
                    oid: (*oid).to_owned(),
                },
                edit: None,
            }
            .save_in_repo(&repo)
            .unwrap();
        }

        let mut args = vec!["query", "find"];
        if use_assignee {
            args.push("--assignee");
            args.push("alice");
        }
        if use_reviewer {
            args.push("--reviewer");
            args.push("carol");
        }
        if use_requester {
            args.push("--requester");
            args.push("dave");
        }
        match text_filter_kind {
            1 => {
                args.push("--keyword");
                args.push("release");
            }
            2 => {
                args.push("--title");
                args.push("release");
            }
            _ => {}
        }

        let (out, err, ok) = run(path, &args);
        prop_assert!(ok, "query find {:?} failed: {err}", &args[2..]);

        let use_text_filter = text_filter_kind != 0;
        let mut expected = Vec::new();

        for (id, body, assignee) in &issue_rows {
            let matches_assignee = !use_assignee || *assignee == "alice";
            let matches_kind_specific = !use_reviewer && !use_requester;
            let matches_text = !use_text_filter || body.contains("release");
            if matches_assignee && matches_kind_specific && matches_text {
                expected.push(format!("issue {id}"));
            }
        }

        for (id, body, reviewer, requester, _) in &review_rows {
            let matches_assignee = !use_assignee;
            let matches_reviewer = !use_reviewer || *reviewer == "carol";
            let matches_requester = !use_requester || *requester == "dave";
            let matches_text = !use_text_filter || body.contains("release");
            if matches_assignee && matches_reviewer && matches_requester && matches_text {
                expected.push(format!("review {id}"));
            }
        }

        let got = bulleted_items(&out);
        let oracle = "oracle: issues where assignee/alice if set, no reviewer/requester filters, text has release if set; reviews where no assignee filter, reviewer/requester match if set, text has release if set";
        prop_assert_eq!(
            got,
            expected,
            "unexpected output for args {:?}; {}",
            &args[2..],
            oracle
        );
    }
}
