//! `search`: convenience for callers who don't know the query language.
//!
//! Every function here compiles its filters to Datalog goal text and runs it
//! through [`crate::facts::run_forge_goal`] -- the exact path `query` takes
//! (`gix_query::run_goal_with` over [`crate::facts::ForgeFacts`]). None of
//! these loop over `Entity::list` themselves; the CLI must not either.
//!
//! This replaces the in-memory scans that used to live in the `git-forge`
//! binary (`QueryCommand::Assignee/Reviewer/Requester/Keyword/Find`):
//! same semantics, compiled to goals instead of hand-rolled loops.
//! Formatting -- e.g. rendering a hit as `"issue 42"` -- is left to the
//! caller: the library is data, not rendering.

use gix::Repository;

use crate::error::Error;
use crate::facts::{quote, run_forge_goal};

/// Which entity kind a [`SearchHit`] names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HitKind {
    Issue,
    Review,
}

impl HitKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            HitKind::Issue => "issue",
            HitKind::Review => "review",
        }
    }
}

/// One search result: which kind of entity, and its id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchHit {
    pub kind: HitKind,
    pub id: String,
}

/// Issues assigned to `name`.
///
/// # Errors
/// See [`Error`].
pub fn search_assignee(repo: &Repository, name: &str) -> Result<Vec<String>, Error> {
    ids_for_goal(repo, &format!("issue_assignee(Id, {})", quote(name)))
}

/// Reviews with `name` as a reviewer.
///
/// # Errors
/// See [`Error`].
pub fn search_reviewer(repo: &Repository, name: &str) -> Result<Vec<String>, Error> {
    ids_for_goal(repo, &format!("review_reviewer(Id, {})", quote(name)))
}

/// Reviews with `name` as a requester.
///
/// # Errors
/// See [`Error`].
pub fn search_requester(repo: &Repository, name: &str) -> Result<Vec<String>, Error> {
    ids_for_goal(repo, &format!("review_requester(Id, {})", quote(name)))
}

/// Issues and reviews whose body contains `value`, case-insensitively.
///
/// # Errors
/// See [`Error`].
pub fn search_keyword(repo: &Repository, value: &str) -> Result<Vec<SearchHit>, Error> {
    let mut hits = Vec::new();
    for id in search_issue(repo, None, Some(value))? {
        hits.push(SearchHit {
            kind: HitKind::Issue,
            id,
        });
    }
    for id in search_review(repo, None, None, Some(value))? {
        hits.push(SearchHit {
            kind: HitKind::Review,
            id,
        });
    }
    Ok(hits)
}

/// Issues matched by `assignee` and/or a case-insensitive `keyword` in the
/// body -- the filters `issue search` exposes.
///
/// # Errors
/// See [`Error`].
pub fn search_issue(
    repo: &Repository,
    assignee: Option<&str>,
    keyword: Option<&str>,
) -> Result<Vec<String>, Error> {
    let mut clauses = Vec::new();
    match assignee {
        Some(name) => clauses.push(format!("issue_assignee(Id, {})", quote(name))),
        None => clauses.push("issue(Id)".to_owned()),
    }
    if let Some(value) = keyword {
        clauses.push(format!(
            "issue_body_contains(Id, {})",
            quote(&value.to_ascii_lowercase())
        ));
    }
    ids_for_goal(repo, &clauses.join(", "))
}

/// Reviews matched by `reviewer`, `requester`, and/or a case-insensitive
/// `keyword` in the body -- the filters `review search` exposes.
///
/// # Errors
/// See [`Error`].
pub fn search_review(
    repo: &Repository,
    reviewer: Option<&str>,
    requester: Option<&str>,
    keyword: Option<&str>,
) -> Result<Vec<String>, Error> {
    let mut clauses = Vec::new();
    if let Some(name) = reviewer {
        clauses.push(format!("review_reviewer(Id, {})", quote(name)));
    }
    if let Some(name) = requester {
        clauses.push(format!("review_requester(Id, {})", quote(name)));
    }
    if clauses.is_empty() {
        clauses.push("review(Id)".to_owned());
    }
    if let Some(value) = keyword {
        clauses.push(format!(
            "review_body_contains(Id, {})",
            quote(&value.to_ascii_lowercase())
        ));
    }
    ids_for_goal(repo, &clauses.join(", "))
}

/// Comments matched by `author` and/or a case-insensitive `keyword` in the
/// body -- the filters `comment search` exposes.
///
/// # Errors
/// See [`Error`].
pub fn search_comment(
    repo: &Repository,
    author: Option<&str>,
    keyword: Option<&str>,
) -> Result<Vec<String>, Error> {
    let mut clauses = Vec::new();
    match author {
        Some(name) => clauses.push(format!("comment_author(Id, {})", quote(name))),
        None => clauses.push("comment(Id)".to_owned()),
    }
    if let Some(value) = keyword {
        clauses.push(format!(
            "comment_body_contains(Id, {})",
            quote(&value.to_ascii_lowercase())
        ));
    }
    ids_for_goal(repo, &clauses.join(", "))
}

/// The combined filter `QueryCommand::Find` used to expose: issues matched by
/// `assignee` and/or `keyword`, reviews matched by `reviewer`/`requester`
/// and/or `keyword` -- with the same mutual-exclusion the original command
/// had: supplying `reviewer` or `requester` excludes issues from the result,
/// and supplying `assignee` excludes reviews, since neither entity has the
/// other's fields to filter on.
///
/// # Errors
/// See [`Error`].
pub fn search_find(
    repo: &Repository,
    assignee: Option<&str>,
    reviewer: Option<&str>,
    requester: Option<&str>,
    keyword: Option<&str>,
) -> Result<Vec<SearchHit>, Error> {
    let mut hits = Vec::new();

    if reviewer.is_none() && requester.is_none() {
        for id in search_issue(repo, assignee, keyword)? {
            hits.push(SearchHit {
                kind: HitKind::Issue,
                id,
            });
        }
    }

    if assignee.is_none() {
        for id in search_review(repo, reviewer, requester, keyword)? {
            hits.push(SearchHit {
                kind: HitKind::Review,
                id,
            });
        }
    }

    Ok(hits)
}

fn ids_for_goal(repo: &Repository, goal: &str) -> Result<Vec<String>, Error> {
    Ok(run_forge_goal(repo, goal, &["Id"])?
        .into_iter()
        .map(|row| row[0].to_string())
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::comment::Comment;
    use crate::entity::EntityOps;
    use crate::issue::Issue;
    use crate::review::{Review, ReviewTarget};

    fn sample_repo() -> (tempfile::TempDir, Repository) {
        let dir = tempfile::tempdir().expect("tempdir");
        test_support::init_repo(dir.path());
        let repo = gix::open(dir.path()).expect("open repo");
        (dir, repo)
    }

    fn issue(body: &str, assignees: &[&str]) -> Issue {
        Issue {
            id: String::new(),
            status: "open".to_string(),
            title: "t".to_string(),
            body: body.to_string(),
            labels: vec![],
            assignees: assignees.iter().map(|s| (*s).to_string()).collect(),
            reporters: vec![],
            edit: None,
        }
    }

    fn review(body: &str, reviewers: &[&str], requesters: &[&str]) -> Review {
        Review {
            id: String::new(),
            status: "open".to_string(),
            body: body.to_string(),
            reviewers: reviewers.iter().map(|s| (*s).to_string()).collect(),
            requesters: requesters.iter().map(|s| (*s).to_string()).collect(),
            target: ReviewTarget::Commit {
                oid: gix::ObjectId::null(gix::hash::Kind::Sha1).to_string(),
            },
            edit: None,
        }
    }

    #[test]
    fn search_assignee_finds_only_matching_issues() {
        let (_dir, repo) = sample_repo();
        let matching = issue("a", &["alice"])
            .create_in_repo(&repo)
            .expect("create");
        issue("b", &["bob"]).create_in_repo(&repo).expect("create");

        let hits = search_assignee(&repo, "alice").expect("search");
        assert_eq!(hits, vec![matching]);
    }

    #[test]
    fn search_keyword_covers_both_issues_and_reviews() {
        let (_dir, repo) = sample_repo();
        let issue_id = issue("has the Widget bug", &[])
            .create_in_repo(&repo)
            .expect("create issue");
        let review_id = review("touches the widget code", &[], &[])
            .create_in_repo(&repo)
            .expect("create review");

        let mut hits = search_keyword(&repo, "widget").expect("search");
        hits.sort_by(|a, b| a.id.cmp(&b.id));
        let mut expected = vec![
            SearchHit {
                kind: HitKind::Issue,
                id: issue_id,
            },
            SearchHit {
                kind: HitKind::Review,
                id: review_id,
            },
        ];
        expected.sort_by(|a, b| a.id.cmp(&b.id));
        assert_eq!(hits, expected);
    }

    #[test]
    fn search_find_excludes_issues_when_a_review_filter_is_given() {
        let (_dir, repo) = sample_repo();
        issue("a", &["alice"])
            .create_in_repo(&repo)
            .expect("create");
        let review_id = review("b", &["carol"], &[])
            .create_in_repo(&repo)
            .expect("create review");

        let hits = search_find(&repo, None, Some("carol"), None, None).expect("search");
        assert_eq!(
            hits,
            vec![SearchHit {
                kind: HitKind::Review,
                id: review_id,
            }]
        );
    }

    #[test]
    fn search_find_excludes_reviews_when_assignee_is_given() {
        let (_dir, repo) = sample_repo();
        let issue_id = issue("a", &["alice"])
            .create_in_repo(&repo)
            .expect("create");
        review("b", &["carol"], &[])
            .create_in_repo(&repo)
            .expect("create review");

        let hits = search_find(&repo, Some("alice"), None, None, None).expect("search");
        assert_eq!(
            hits,
            vec![SearchHit {
                kind: HitKind::Issue,
                id: issue_id,
            }]
        );
    }

    #[test]
    fn search_issue_combines_assignee_and_keyword() {
        let (_dir, repo) = sample_repo();
        let matching = issue("has the widget bug", &["alice"])
            .create_in_repo(&repo)
            .expect("create");
        issue("has the widget bug", &["bob"])
            .create_in_repo(&repo)
            .expect("create");
        issue("unrelated", &["alice"])
            .create_in_repo(&repo)
            .expect("create");

        let ids = search_issue(&repo, Some("alice"), Some("widget")).expect("search");
        assert_eq!(ids, vec![matching]);
    }

    #[test]
    fn search_issue_with_no_filters_lists_every_issue() {
        let (_dir, repo) = sample_repo();
        let id = issue("a", &[]).create_in_repo(&repo).expect("create");

        let ids = search_issue(&repo, None, None).expect("search");
        assert_eq!(ids, vec![id]);
    }

    #[test]
    fn search_review_combines_reviewer_requester_and_keyword() {
        let (_dir, repo) = sample_repo();
        let matching = review("please review the widget", &["carol"], &["dave"])
            .create_in_repo(&repo)
            .expect("create");
        review("please review the widget", &["carol"], &["erin"])
            .create_in_repo(&repo)
            .expect("create");

        let ids =
            search_review(&repo, Some("carol"), Some("dave"), Some("widget")).expect("search");
        assert_eq!(ids, vec![matching]);
    }

    #[test]
    fn search_comment_combines_author_and_keyword() {
        let (_dir, repo) = sample_repo();
        let issue_id = issue("a", &[]).create_in_repo(&repo).expect("create issue");
        let matching =
            Comment::create_under(&repo, "issue", &issue_id, "alice", "the widget broke", None)
                .expect("create comment");
        Comment::create_under(&repo, "issue", &issue_id, "bob", "the widget broke", None)
            .expect("create comment");
        Comment::create_under(&repo, "issue", &issue_id, "alice", "unrelated", None)
            .expect("create comment");

        let ids = search_comment(&repo, Some("alice"), Some("widget")).expect("search");
        assert_eq!(ids, vec![matching]);
    }

    #[test]
    fn search_comment_with_no_filters_lists_every_comment() {
        let (_dir, repo) = sample_repo();
        let issue_id = issue("a", &[]).create_in_repo(&repo).expect("create issue");
        let id = Comment::create_under(&repo, "issue", &issue_id, "alice", "b", None)
            .expect("create comment");

        let ids = search_comment(&repo, None, None).expect("search");
        assert_eq!(ids, vec![id]);
    }
}
