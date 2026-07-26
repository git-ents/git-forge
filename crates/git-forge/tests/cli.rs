//! Drive the built `git-forge` binary against a temp repo, exactly as
//! `git forge …` would.

use std::path::Path;
use std::process::{Command, Stdio};

use gix_forge::{Issue, Review, ReviewTarget};
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

fn put_issue(dir: &Path, id: &str, body: &str) {
    let repo = gix::open(dir).unwrap();
    let issue = Issue {
        id: id.to_owned(),
        body: body.to_owned(),
        labels: vec![],
        assignees: vec![],
        reporters: vec![],
    };
    issue.save_in_repo(&repo).unwrap();
}

fn put_review(dir: &Path, id: &str, body: &str) {
    let repo = gix::open(dir).unwrap();
    let review = Review {
        id: id.to_owned(),
        body: body.to_owned(),
        reviewers: vec![],
        requesters: vec![],
        target: ReviewTarget::Commit {
            oid: "deadbeef".to_owned(),
        },
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

    let show_args = vec!["issue", "show", issue_id.as_str()];
    let (out, err, ok) = run(path, &show_args);
    assert!(ok, "issue show failed: {err}");
    assert!(
        out.contains(&format!("id: {issue_id}")),
        "issue show output: {out}"
    );
    assert!(out.contains("body: first body"), "issue show output: {out}");
    assert!(out.contains("labels: bug,p1"), "issue show output: {out}");
    assert!(out.contains("assignees: alice"), "issue show output: {out}");
    assert!(out.contains("reporters: bob"), "issue show output: {out}");

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
    assert!(out.contains("id: abc11111"), "issue show output: {out}");
    assert!(out.contains("body: first body"), "issue show output: {out}");

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
fn review_show_accepts_min_unique_prefix_and_renders_ambiguous_matches() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    let path = dir.path();

    put_review(path, "def11111", "first review");
    put_review(path, "def22222", "second review");

    let (out, err, ok) = run(path, &["review", "show", "def1"]);
    assert!(ok, "review show by unique prefix failed: {err}");
    assert!(out.contains("id: def11111"), "review show output: {out}");
    assert!(
        out.contains("body: first review"),
        "review show output: {out}"
    );

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
fn review_new_show_list_log_and_remove() {
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

    let show_args = vec!["review", "show", review_id.as_str()];
    let (out, err, ok) = run(path, &show_args);
    assert!(ok, "review show failed: {err}");
    assert!(
        out.contains(&format!("id: {review_id}")),
        "review show output: {out}"
    );
    assert!(
        out.contains("body: looks good"),
        "review show output: {out}"
    );
    assert!(
        out.contains("reviewers: carol"),
        "review show output: {out}"
    );
    assert!(
        out.contains("requesters: dave"),
        "review show output: {out}"
    );
    assert!(
        out.contains("target: commit:deadbeef"),
        "review show output: {out}"
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

    let (_, _, ok) = run(path, &show_args);
    assert!(!ok, "review show after rm should fail");
}

#[test]
fn query_sugar_filters_by_people_and_keyword() {
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
            "release blocker",
            "--assignee",
            "alice",
            "--reporter",
            "bob",
        ],
    );
    assert!(ok, "issue new failed: {err}");

    let review_id = create_origin_commit(path);
    let (_, err, ok) = run(
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

    let (out, err, ok) = run(path, &["query", "assignee", "alice"]);
    assert!(ok, "query assignee failed: {err}");
    assert_eq!(out.trim(), issue_id);

    let (out, err, ok) = run(path, &["query", "reviewer", "carol"]);
    assert!(ok, "query reviewer failed: {err}");
    assert_eq!(out.trim(), review_id);

    let (out, err, ok) = run(path, &["query", "requester", "dave"]);
    assert!(ok, "query requester failed: {err}");
    assert_eq!(out.trim(), review_id);

    let (out, err, ok) = run(path, &["query", "keyword", "release"]);
    assert!(ok, "query keyword failed: {err}");
    assert!(
        out.contains(&format!("issue:{issue_id}")),
        "query keyword output: {out}"
    );
    assert!(
        out.contains(&format!("review:{review_id}")),
        "query keyword output: {out}"
    );

    let (out, err, ok) = run(path, &["query", "title", "reviewed"]);
    assert!(ok, "query title alias failed: {err}");
    assert_eq!(out.trim(), format!("review:{review_id}"));

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
    assert_eq!(out.trim(), format!("issue:{issue_id}"));

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
    assert_eq!(out.trim(), format!("review:{review_id}"));

    let (_, err, ok) = run(path, &["query", "find"]);
    assert!(!ok, "query find without filters should fail");
    assert!(
        err.contains("requires at least one filter"),
        "query find without filters stderr: {err}"
    );
}

#[test]
fn query_find_supports_all_filter_combinations() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    let path = dir.path();

    let repo = gix::open(path).unwrap();

    let matching_issue_id = "iss11111";
    Issue {
        id: matching_issue_id.to_owned(),
        body: "release blocker".to_owned(),
        labels: vec![],
        assignees: vec!["alice".to_owned()],
        reporters: vec![],
    }
    .save_in_repo(&repo)
    .unwrap();

    Issue {
        id: "iss22222".to_owned(),
        body: "misc task".to_owned(),
        labels: vec![],
        assignees: vec!["eve".to_owned()],
        reporters: vec![],
    }
    .save_in_repo(&repo)
    .unwrap();

    let matching_review_id = "rev11111";
    Review {
        id: matching_review_id.to_owned(),
        body: "release reviewed".to_owned(),
        reviewers: vec!["carol".to_owned()],
        requesters: vec!["dave".to_owned()],
        target: ReviewTarget::Commit {
            oid: "deadbeef".to_owned(),
        },
    }
    .save_in_repo(&repo)
    .unwrap();

    Review {
        id: "rev22222".to_owned(),
        body: "other note".to_owned(),
        reviewers: vec!["mallory".to_owned()],
        requesters: vec!["trent".to_owned()],
        target: ReviewTarget::Commit {
            oid: "feedface".to_owned(),
        },
    }
    .save_in_repo(&repo)
    .unwrap();

    for use_assignee in [false, true] {
        for use_reviewer in [false, true] {
            for use_requester in [false, true] {
                for text_filter in [None, Some("keyword"), Some("title")] {
                    if !use_assignee && !use_reviewer && !use_requester && text_filter.is_none() {
                        continue;
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
                    match text_filter {
                        Some("keyword") => {
                            args.push("--keyword");
                            args.push("release");
                        }
                        Some("title") => {
                            args.push("--title");
                            args.push("release");
                        }
                        _ => {}
                    }

                    let (out, err, ok) = run(path, &args);
                    assert!(ok, "query find {:?} failed: {err}", &args[2..],);

                    let issue_matches = !use_reviewer && !use_requester;
                    let review_matches = !use_assignee;

                    let mut expected = Vec::new();
                    if issue_matches {
                        expected.push(format!("issue:{matching_issue_id}"));
                    }
                    if review_matches {
                        expected.push(format!("review:{matching_review_id}"));
                    }

                    let got: Vec<&str> = out.lines().filter(|line| !line.is_empty()).collect();
                    let expected_refs: Vec<&str> = expected.iter().map(String::as_str).collect();
                    assert_eq!(
                        got,
                        expected_refs,
                        "unexpected output for args {:?}",
                        &args[2..]
                    );
                }
            }
        }
    }
}
