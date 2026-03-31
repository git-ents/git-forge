//! Integration tests for comment chains.
#![allow(
    clippy::must_use_candidate,
    clippy::missing_panics_doc,
    missing_docs,
    deprecated
)]

use git2::Repository;
use tempfile::TempDir;

use git_forge::Store;
use git_forge::comment::list_thread;
use git_forge::comment::{
    Anchor, LegacyAnchor, add_comment, add_reply, comment_thread_ref, create_thread, edit_comment,
    edit_in_thread, find_threads_by_object, index_lookup, issue_comment_ref, list_comments,
    list_thread_comments, rebuild_comments_index, reply_to_thread, resolve_comment, resolve_thread,
};
use git_forge::exe::Executor;

fn test_repo() -> (TempDir, Repository) {
    let dir = TempDir::new().expect("temp dir");
    let repo = Repository::init(dir.path()).expect("init repo");
    {
        let mut cfg = repo.config().expect("config");
        cfg.set_str("user.name", "test").expect("user.name");
        cfg.set_str("user.email", "test@test.com")
            .expect("user.email");
    }
    {
        let sig = git2::Signature::now("test", "test@test.com").expect("sig");
        let mut index = repo.index().expect("index");
        let tree_oid = index.write_tree().expect("write tree");
        let tree = repo.find_tree(tree_oid).expect("find tree");
        repo.commit(Some("HEAD"), &sig, &sig, "initial", &tree, &[])
            .expect("initial commit");
    }
    (dir, repo)
}

#[test]
fn add_comment_creates_chain() {
    let (_dir, repo) = test_repo();
    let ref_name = issue_comment_ref("abc123");
    let comment = add_comment(&repo, &ref_name, "first comment", None).unwrap();

    assert_eq!(comment.body, "first comment");
    assert_eq!(comment.author_name, "test");
    assert_eq!(comment.author_email, "test@test.com");
    assert!(!comment.resolved);
    assert!(comment.replaces.is_none());
    assert!(comment.reply_to.is_none());
    assert!(comment.anchor.is_none());
    assert_eq!(comment.oid.len(), 40);
}

#[test]
fn add_second_comment() {
    let (_dir, repo) = test_repo();
    let ref_name = issue_comment_ref("abc123");
    let c1 = add_comment(&repo, &ref_name, "first", None).unwrap();
    let c2 = add_comment(&repo, &ref_name, "second", None).unwrap();

    assert_ne!(c1.oid, c2.oid);
    assert_eq!(c2.body, "second");
    assert!(c2.reply_to.is_none());
}

#[test]
fn reply_threading() {
    let (_dir, repo) = test_repo();
    let ref_name = issue_comment_ref("abc123");
    let root = add_comment(&repo, &ref_name, "root comment", None).unwrap();
    let reply = add_reply(&repo, &ref_name, "reply text", &root.oid, None).unwrap();

    assert_eq!(reply.reply_to.as_deref(), Some(root.oid.as_str()));
    assert_eq!(reply.body, "reply text");
}

#[test]
fn list_comments_chronological() {
    let (_dir, repo) = test_repo();
    let ref_name = issue_comment_ref("abc123");
    add_comment(&repo, &ref_name, "alpha", None).unwrap();
    add_comment(&repo, &ref_name, "beta", None).unwrap();
    add_comment(&repo, &ref_name, "gamma", None).unwrap();

    let comments = list_comments(&repo, &ref_name).unwrap();
    // walk returns tip-first (reverse chronological)
    assert_eq!(comments.len(), 3);
    assert_eq!(comments[0].body, "gamma");
    assert_eq!(comments[2].body, "alpha");
}

#[test]
fn list_comments_in_thread() {
    let (_dir, repo) = test_repo();
    let ref_name = issue_comment_ref("abc123");
    let root = add_comment(&repo, &ref_name, "root", None).unwrap();
    let reply = add_reply(&repo, &ref_name, "reply", &root.oid, None).unwrap();
    // unrelated top-level comment
    add_comment(&repo, &ref_name, "unrelated", None).unwrap();

    let thread = git_forge::comment::list_thread(&repo, &ref_name, &root.oid).unwrap();
    let oids: Vec<&str> = thread.iter().map(|c| c.oid.as_str()).collect();
    assert!(oids.contains(&root.oid.as_str()));
    assert!(oids.contains(&reply.oid.as_str()));
    assert_eq!(thread.len(), 2);
}

#[test]
fn resolve_sets_trailer() {
    let (_dir, repo) = test_repo();
    let ref_name = issue_comment_ref("abc123");
    let root = add_comment(&repo, &ref_name, "needs resolve", None).unwrap();
    let resolution = resolve_comment(&repo, &ref_name, &root.oid, None).unwrap();

    assert!(resolution.resolved);
    assert_eq!(resolution.reply_to.as_deref(), Some(root.oid.as_str()));
}

#[test]
fn edit_sets_replaces() {
    let (_dir, repo) = test_repo();
    let ref_name = issue_comment_ref("abc123");
    let original = add_comment(&repo, &ref_name, "original text", None).unwrap();
    let edited = edit_comment(&repo, &ref_name, &original.oid, "updated text", None).unwrap();

    assert_eq!(edited.body, "updated text");
    assert_eq!(edited.replaces.as_deref(), Some(original.oid.as_str()));
}

#[test]
fn anchor_object_with_range() {
    let (_dir, repo) = test_repo();
    let ref_name = issue_comment_ref("abc123");
    let anchor = LegacyAnchor::Object {
        oid: "asdfhjkl".to_string(),
        path: None,
        range: Some("10-20".to_string()),
    };
    let comment = add_comment(&repo, &ref_name, "line comment", Some(&anchor)).unwrap();

    let a = comment.anchor.as_ref().unwrap();
    match a {
        LegacyAnchor::Object { oid, range, .. } => {
            assert_eq!(oid, "asdfhjkl");
            assert_eq!(range.as_deref(), Some("10-20"));
        }
        LegacyAnchor::CommitRange { .. } => panic!("expected Object anchor"),
    }
}

#[test]
fn anchor_commit_range() {
    let (_dir, repo) = test_repo();
    let ref_name = issue_comment_ref("abc123");
    let anchor = LegacyAnchor::CommitRange {
        start: "aaa".to_string(),
        end: "bbb".to_string(),
    };
    let comment = add_comment(&repo, &ref_name, "range comment", Some(&anchor)).unwrap();

    let a = comment.anchor.as_ref().unwrap();
    match a {
        LegacyAnchor::CommitRange { start, end } => {
            assert_eq!(start, "aaa");
            assert_eq!(end, "bbb");
        }
        LegacyAnchor::Object { .. } => panic!("expected CommitRange anchor"),
    }
}

#[test]
fn body_with_trailer_like_text_survives_roundtrip() {
    let (_dir, repo) = test_repo();
    let ref_name = issue_comment_ref("abc123");
    let body = "The fix is: set Key: value in the config\nSigned-off-by: someone";
    let comment = add_comment(&repo, &ref_name, body, None).unwrap();

    let comments = list_comments(&repo, &ref_name).unwrap();
    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].body, body);
    assert_eq!(comments[0].oid, comment.oid);
}

#[test]
fn list_comments_empty_chain() {
    let (_dir, repo) = test_repo();
    let ref_name = issue_comment_ref("nonexistent");
    let comments = list_comments(&repo, &ref_name).unwrap();
    assert!(comments.is_empty());
}

#[test]
fn resolve_comment_with_message() {
    let (_dir, repo) = test_repo();
    let ref_name = issue_comment_ref("abc123");
    let root = add_comment(&repo, &ref_name, "needs work", None).unwrap();
    let resolution = resolve_comment(
        &repo,
        &ref_name,
        &root.oid,
        Some("addressed in latest push"),
    )
    .unwrap();

    assert!(resolution.resolved);
    assert_eq!(resolution.body, "addressed in latest push");
    assert_eq!(resolution.reply_to.as_deref(), Some(root.oid.as_str()));
}

#[test]
fn deep_thread_reply_to_reply() {
    let (_dir, repo) = test_repo();
    let ref_name = issue_comment_ref("abc123");
    let root = add_comment(&repo, &ref_name, "root", None).unwrap();
    let child = add_reply(&repo, &ref_name, "child", &root.oid, None).unwrap();
    let grandchild = add_reply(&repo, &ref_name, "grandchild", &child.oid, None).unwrap();
    // unrelated top-level comment should not appear in the thread
    add_comment(&repo, &ref_name, "unrelated", None).unwrap();

    let thread = git_forge::comment::list_thread(&repo, &ref_name, &root.oid).unwrap();
    let oids: Vec<&str> = thread.iter().map(|c| c.oid.as_str()).collect();
    assert_eq!(thread.len(), 3);
    assert!(oids.contains(&root.oid.as_str()));
    assert!(oids.contains(&child.oid.as_str()));
    assert!(oids.contains(&grandchild.oid.as_str()));
    assert_eq!(grandchild.reply_to.as_deref(), Some(child.oid.as_str()));
}

#[test]
fn executor_add_and_list_comments() {
    let (_dir, repo) = test_repo();
    let store = Store::new(&repo);
    let issue = store.create_issue("Test issue", "body", &[], &[]).unwrap();
    let exec = Executor::from_path(repo.path().parent().unwrap()).unwrap();

    let c1 = exec.add_issue_comment(&issue.oid, "first", None).unwrap();
    let c2 = exec.add_issue_comment(&issue.oid, "second", None).unwrap();

    let comments = exec.list_issue_comments(&issue.oid).unwrap();
    assert_eq!(comments.len(), 2);
    let oids: Vec<&str> = comments.iter().map(|c| c.oid.as_str()).collect();
    assert!(oids.contains(&c1.oid.as_str()));
    assert!(oids.contains(&c2.oid.as_str()));
}

#[test]
fn executor_reply_and_resolve() {
    let (_dir, repo) = test_repo();
    let store = Store::new(&repo);
    let issue = store.create_issue("Test issue", "body", &[], &[]).unwrap();
    let exec = Executor::from_path(repo.path().parent().unwrap()).unwrap();

    let root = exec.add_issue_comment(&issue.oid, "root", None).unwrap();
    let reply = exec
        .reply_issue_comment(&issue.oid, "reply text", &root.oid, None)
        .unwrap();
    assert_eq!(reply.reply_to.as_deref(), Some(root.oid.as_str()));

    let resolved = exec
        .resolve_issue_comment(&issue.oid, &root.oid, Some("done"))
        .unwrap();
    assert!(resolved.resolved);
    assert_eq!(resolved.body, "done");
}

#[test]
fn executor_comment_on_nonexistent_issue() {
    let (_dir, repo) = test_repo();
    let exec = Executor::from_path(repo.path().parent().unwrap()).unwrap();

    let result = exec.add_issue_comment("nonexistent", "body", None);
    assert!(result.is_err());
}

#[test]
fn edit_preserves_anchor() {
    let (_dir, repo) = test_repo();
    let ref_name = issue_comment_ref("abc123");
    let anchor = LegacyAnchor::Object {
        oid: "asdfhjkl".to_string(),
        path: None,
        range: Some("5-10".to_string()),
    };
    let original = add_comment(&repo, &ref_name, "original", Some(&anchor)).unwrap();
    let edited = edit_comment(&repo, &ref_name, &original.oid, "edited", Some(&anchor)).unwrap();

    assert_eq!(edited.body, "edited");
    assert_eq!(edited.replaces.as_deref(), Some(original.oid.as_str()));
    match edited.anchor.as_ref().unwrap() {
        LegacyAnchor::Object { oid, range, .. } => {
            assert_eq!(oid, "asdfhjkl");
            assert_eq!(range.as_deref(), Some("5-10"));
        }
        LegacyAnchor::CommitRange { .. } => panic!("expected Object anchor"),
    }
}

// --- OID prefix resolution tests ---

#[test]
fn reply_accepts_oid_prefix() {
    let (_dir, repo) = test_repo();
    let ref_name = issue_comment_ref("abc123");
    let root = add_comment(&repo, &ref_name, "root", None).unwrap();

    let prefix = &root.oid[..8];
    let reply = add_reply(&repo, &ref_name, "prefix reply", prefix, None).unwrap();
    assert_eq!(reply.reply_to.as_deref(), Some(root.oid.as_str()));
}

#[test]
fn resolve_accepts_oid_prefix() {
    let (_dir, repo) = test_repo();
    let ref_name = issue_comment_ref("abc123");
    let root = add_comment(&repo, &ref_name, "root", None).unwrap();

    let prefix = &root.oid[..8];
    let resolution = resolve_comment(&repo, &ref_name, prefix, Some("done")).unwrap();
    assert!(resolution.resolved);
    assert_eq!(resolution.reply_to.as_deref(), Some(root.oid.as_str()));
}

#[test]
fn edit_accepts_oid_prefix() {
    let (_dir, repo) = test_repo();
    let ref_name = issue_comment_ref("abc123");
    let original = add_comment(&repo, &ref_name, "original", None).unwrap();

    let prefix = &original.oid[..8];
    let edited = edit_comment(&repo, &ref_name, prefix, "edited", None).unwrap();
    assert_eq!(edited.body, "edited");
    assert_eq!(edited.replaces.as_deref(), Some(original.oid.as_str()));
}

#[test]
fn list_thread_accepts_oid_prefix() {
    let (_dir, repo) = test_repo();
    let ref_name = issue_comment_ref("abc123");
    let root = add_comment(&repo, &ref_name, "root", None).unwrap();
    add_reply(&repo, &ref_name, "reply", &root.oid, None).unwrap();
    add_comment(&repo, &ref_name, "unrelated", None).unwrap();

    let prefix = &root.oid[..8];
    let thread = list_thread(&repo, &ref_name, prefix).unwrap();
    assert_eq!(thread.len(), 2);
}

#[test]
fn oid_prefix_not_found_errors() {
    let (_dir, repo) = test_repo();
    let ref_name = issue_comment_ref("abc123");
    add_comment(&repo, &ref_name, "root", None).unwrap();

    let result = add_reply(&repo, &ref_name, "reply", "asdfhjkl", None);
    assert!(result.is_err());
}

// --- Anchor path roundtrip (issue 1 regression) ---

#[test]
fn anchor_object_with_path_roundtrips() {
    let (_dir, repo) = test_repo();
    let ref_name = issue_comment_ref("abc123");
    let anchor = LegacyAnchor::Object {
        oid: "abc123".to_string(),
        path: Some("src/main.rs".to_string()),
        range: Some("42-47".to_string()),
    };
    let comment = add_comment(&repo, &ref_name, "path comment", Some(&anchor)).unwrap();

    match comment.anchor.as_ref().unwrap() {
        LegacyAnchor::Object { oid, path, range } => {
            assert_eq!(oid, "abc123");
            assert_eq!(path.as_deref(), Some("src/main.rs"));
            assert_eq!(range.as_deref(), Some("42-47"));
        }
        LegacyAnchor::CommitRange { .. } => panic!("expected Object anchor"),
    }

    // Also verify via list_comments roundtrip.
    let all = list_comments(&repo, &ref_name).unwrap();
    match all[0].anchor.as_ref().unwrap() {
        LegacyAnchor::Object { path, .. } => assert_eq!(path.as_deref(), Some("src/main.rs")),
        LegacyAnchor::CommitRange { .. } => panic!("expected Object anchor"),
    }
}

// --- v2 API tests ---

#[test]
fn create_thread_produces_ref() {
    let (_dir, repo) = test_repo();
    let blob_oid = repo.blob(b"anchor content").unwrap().to_string();
    let anchor = Anchor {
        oid: blob_oid,
        start_line: Some(10),
        end_line: Some(20),
    };
    let (thread_id, root) = create_thread(&repo, "first comment", Some(&anchor), None).unwrap();
    assert!(!thread_id.is_empty());
    assert_eq!(root.body, "first comment");

    // Ref must exist in the repo.
    let ref_name = comment_thread_ref(&thread_id);
    assert!(repo.find_reference(&ref_name).is_ok());
}

#[test]
fn thread_tree_roundtrip() {
    let (_dir, repo) = test_repo();
    let blob_oid = repo.blob(b"source file content").unwrap().to_string();
    let anchor = Anchor {
        oid: blob_oid.clone(),
        start_line: Some(1),
        end_line: None,
    };
    let (thread_id, _) = create_thread(&repo, "body text", Some(&anchor), None).unwrap();

    let comments = list_thread_comments(&repo, &thread_id).unwrap();
    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].body, "body text");
    match comments[0].anchor.as_ref().unwrap() {
        LegacyAnchor::Object { oid, .. } => {
            assert_eq!(oid, &blob_oid);
        }
        LegacyAnchor::CommitRange { .. } => panic!("expected Object anchor"),
    }
}

#[test]
fn reply_appends_to_chain() {
    let (_dir, repo) = test_repo();
    let (thread_id, root) = create_thread(&repo, "root", None, None).unwrap();

    let reply = reply_to_thread(&repo, &thread_id, "reply text", &root.oid, None, None).unwrap();
    assert_eq!(reply.reply_to.as_deref(), Some(root.oid.as_str()));

    let all = list_thread_comments(&repo, &thread_id).unwrap();
    assert_eq!(all.len(), 2);
}

#[test]
fn resolve_thread_sets_resolved() {
    let (_dir, repo) = test_repo();
    let (thread_id, root) = create_thread(&repo, "needs work", None, None).unwrap();

    let resolution = resolve_thread(&repo, &thread_id, &root.oid, Some("done")).unwrap();
    assert!(resolution.resolved);
    assert_eq!(resolution.body, "done");
    assert_eq!(resolution.reply_to.as_deref(), Some(root.oid.as_str()));
}

#[test]
fn edit_in_thread_sets_replaces() {
    let (_dir, repo) = test_repo();
    let (thread_id, root) = create_thread(&repo, "original", None, None).unwrap();

    let edited = edit_in_thread(&repo, &thread_id, &root.oid, "updated", None, None).unwrap();
    assert_eq!(edited.body, "updated");
    assert_eq!(edited.replaces.as_deref(), Some(root.oid.as_str()));
}

#[test]
fn list_thread_returns_tip_first() {
    let (_dir, repo) = test_repo();
    let (thread_id, root) = create_thread(&repo, "first", None, None).unwrap();
    let r1 = reply_to_thread(&repo, &thread_id, "second", &root.oid, None, None).unwrap();
    reply_to_thread(&repo, &thread_id, "third", &r1.oid, None, None).unwrap();

    let all = list_thread_comments(&repo, &thread_id).unwrap();
    assert_eq!(all.len(), 3);
    // list_thread_comments returns tip-first (reverse chronological)
    assert_eq!(all[0].body, "third");
    assert_eq!(all[2].body, "first");
}

#[test]
fn two_threads_no_contention() {
    let (_dir, repo) = test_repo();
    let (id1, _) = create_thread(&repo, "thread one", None, None).unwrap();
    let (id2, _) = create_thread(&repo, "thread two", None, None).unwrap();

    assert_ne!(id1, id2);

    let c1 = list_thread_comments(&repo, &id1).unwrap();
    let c2 = list_thread_comments(&repo, &id2).unwrap();
    assert_eq!(c1.len(), 1);
    assert_eq!(c2.len(), 1);
    assert_eq!(c1[0].body, "thread one");
    assert_eq!(c2[0].body, "thread two");
}

#[test]
fn comment_id_trailer_consistent() {
    let (_dir, repo) = test_repo();
    let (thread_id, root) = create_thread(&repo, "msg", None, None).unwrap();
    // The comment's oid must match the git commit oid stored in thread_id ref.
    let ref_name = comment_thread_ref(&thread_id);
    let tip = repo
        .find_reference(&ref_name)
        .unwrap()
        .peel_to_commit()
        .unwrap();
    assert_eq!(root.oid, tip.id().to_string());
}

#[test]
fn anchor_trailer_on_every_commit() {
    let (_dir, repo) = test_repo();
    let blob_oid = repo.blob(b"anchor text").unwrap().to_string();
    let anchor = Anchor {
        oid: blob_oid,
        start_line: None,
        end_line: None,
    };
    let (thread_id, root) = create_thread(&repo, "first", Some(&anchor), None).unwrap();
    reply_to_thread(&repo, &thread_id, "second", &root.oid, None, None).unwrap();

    // Every commit in the chain must carry the Anchor trailer.
    let ref_name = comment_thread_ref(&thread_id);
    let mut commit = repo
        .find_reference(&ref_name)
        .unwrap()
        .peel_to_commit()
        .unwrap();
    let mut count = 0;
    loop {
        let msg = commit.message().unwrap_or("");
        assert!(
            msg.contains("Anchor: "),
            "commit missing Anchor trailer: {msg}"
        );
        count += 1;
        if commit.parent_count() == 0 {
            break;
        }
        commit = commit.parent(0).unwrap();
    }
    assert_eq!(count, 2);
}

#[test]
fn rebuild_index_and_lookup() {
    let (_dir, repo) = test_repo();
    let blob_oid = repo.blob(b"indexed file").unwrap().to_string();
    let anchor = Anchor {
        oid: blob_oid.clone(),
        start_line: None,
        end_line: None,
    };
    let (thread_id, _) = create_thread(&repo, "indexed", Some(&anchor), None).unwrap();

    rebuild_comments_index(&repo).unwrap();

    let threads = index_lookup(&repo, &blob_oid).unwrap();
    assert!(threads.is_some());
    let threads = threads.unwrap();
    assert!(threads.contains(&thread_id));
}

#[test]
fn find_threads_by_object_uses_index() {
    let (_dir, repo) = test_repo();
    let blob_oid = repo.blob(b"indexed file 2").unwrap().to_string();
    let anchor = Anchor {
        oid: blob_oid.clone(),
        start_line: Some(5),
        end_line: Some(10),
    };
    let (thread_id, _) = create_thread(&repo, "indexed comment", Some(&anchor), None).unwrap();

    rebuild_comments_index(&repo).unwrap();

    let threads = find_threads_by_object(&repo, &blob_oid).unwrap();
    assert!(threads.contains(&thread_id));
}

#[test]
fn find_threads_fallback_without_index() {
    let (_dir, repo) = test_repo();
    let blob_oid = repo.blob(b"unindexed file").unwrap().to_string();
    let anchor = Anchor {
        oid: blob_oid.clone(),
        start_line: None,
        end_line: None,
    };
    let (thread_id, _) = create_thread(&repo, "unindexed", Some(&anchor), None).unwrap();

    // Do NOT rebuild the index — fallback scan must still find the thread.
    let threads = find_threads_by_object(&repo, &blob_oid).unwrap();
    assert!(threads.contains(&thread_id));
}

#[test]
fn anchor_v2_no_path_field() {
    // v2 Anchor has no `path` field — only oid + line range.
    let anchor = Anchor {
        oid: "abc1230000000000000000000000000000000000".to_string(),
        start_line: Some(1),
        end_line: Some(5),
    };
    assert_eq!(anchor.oid, "abc1230000000000000000000000000000000000");
    assert_eq!(anchor.start_line, Some(1));
    assert_eq!(anchor.end_line, Some(5));
}

#[test]
fn context_lines_roundtrip() {
    let (_dir, repo) = test_repo();
    let context = "fn main() {\n    println!(\"hello\");\n}";
    let blob_oid = repo.blob(context.as_bytes()).unwrap().to_string();
    let anchor = Anchor {
        oid: blob_oid,
        start_line: Some(1),
        end_line: Some(3),
    };
    let (thread_id, _) =
        create_thread(&repo, "context comment", Some(&anchor), Some(context)).unwrap();

    let comments = list_thread_comments(&repo, &thread_id).unwrap();
    assert_eq!(comments[0].context_lines.as_deref(), Some(context));
}
