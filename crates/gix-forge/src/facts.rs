//! [`ForgeFacts`]: the `FactSource` exposing issue, review, and comment
//! fields as EDB predicates, per `gix-query`'s consumer seam
//! (`gix-query/tests/facts_seam.rs`) and ARCHITECTURE.md's
//! "extension-contributed predicates" (`reviewed/1` from forge rules is the
//! example named there; this is the base-fact half that feeds such rules).
//!
//! Every predicate here is read straight off the entities `Issue`, `Review`,
//! and `Comment` already know how to load -- no separate index, no cache.
//! This is the only place forge data is scanned in memory: `search` and
//! `query` both compile to goal text and run through [`run_forge_goal`] /
//! [`run_forge_predicate`], never by looping over `Entity::list` themselves.

use gix::Repository;
use gix_query::{
    ArgSet, Backing, Bindings, Fact, FactSource, Facts, HostError, HostRegistry, PredicateEntry,
    PredicateKey, QueryError, Value, ValueType,
};

use crate::comment::Comment;
use crate::entity::EntityOps;
use crate::issue::Issue;
use crate::review::Review;

/// The EDB names and arities [`ForgeFacts`] answers for -- kept as one list
/// so `provides` and the `facts` dispatch can never name a predicate the
/// other does not (mirrors `gix-query-host::facts::PROVIDED`).
const PROVIDED: &[(&str, usize)] = &[
    ("issue", 1),
    ("issue_status", 2),
    ("issue_assignee", 2),
    ("issue_reporter", 2),
    ("issue_body_contains", 2),
    ("review", 1),
    ("review_status", 2),
    ("review_reviewer", 2),
    ("review_requester", 2),
    ("review_target", 2),
    ("review_body_contains", 2),
    ("comment", 1),
    ("comment_author", 2),
    ("comment_subject", 2),
    ("comment_body_contains", 2),
];

/// A `FactSource` over one repository's issues, reviews, and comments.
pub struct ForgeFacts<'r> {
    repo: &'r Repository,
}

impl<'r> ForgeFacts<'r> {
    #[must_use]
    pub fn new(repo: &'r Repository) -> Self {
        Self { repo }
    }

    /// [`HostRegistry::host`] plus every predicate [`ForgeFacts`] answers
    /// for -- the registry a caller passes to `run_goal_with`,
    /// `run_predicate_with`, or `run_body_with`.
    #[must_use]
    pub fn registry() -> HostRegistry {
        let mut registry = HostRegistry::host();
        for entry in predicate_entries() {
            registry
                .insert_predicate(entry)
                .expect("a forge predicate does not shadow a host one");
        }
        registry
    }
}

fn predicate_entries() -> Vec<PredicateEntry> {
    use ValueType::Symbol;

    let enumerable = ArgSet::empty();
    let keyed_on_needle = ArgSet::from_indices([1]);
    let issues = || vec![Backing::RefGlob("refs/forge/issue/*".into())];
    let reviews = || vec![Backing::RefGlob("refs/forge/review/*".into())];
    let comments = || vec![Backing::RefGlob("refs/forge/comment/*".into())];

    vec![
        PredicateEntry::edb("issue", &[Symbol], enumerable, issues(), "an issue exists"),
        PredicateEntry::edb(
            "issue_status",
            &[Symbol, Symbol],
            enumerable,
            issues(),
            "the issue's status",
        ),
        PredicateEntry::edb(
            "issue_assignee",
            &[Symbol, Symbol],
            enumerable,
            issues(),
            "the issue is assigned to this name",
        ),
        PredicateEntry::edb(
            "issue_reporter",
            &[Symbol, Symbol],
            enumerable,
            issues(),
            "the issue was reported by this name",
        ),
        PredicateEntry::edb(
            "issue_body_contains",
            &[Symbol, Symbol],
            keyed_on_needle,
            issues(),
            "the issue's body contains this needle, case-insensitively",
        ),
        PredicateEntry::edb(
            "review",
            &[Symbol],
            enumerable,
            reviews(),
            "a review exists",
        ),
        PredicateEntry::edb(
            "review_status",
            &[Symbol, Symbol],
            enumerable,
            reviews(),
            "the review's status",
        ),
        PredicateEntry::edb(
            "review_reviewer",
            &[Symbol, Symbol],
            enumerable,
            reviews(),
            "the review has this reviewer",
        ),
        PredicateEntry::edb(
            "review_requester",
            &[Symbol, Symbol],
            enumerable,
            reviews(),
            "the review has this requester",
        ),
        PredicateEntry::edb(
            "review_target",
            &[Symbol, Symbol],
            enumerable,
            reviews(),
            "the review's formatted target",
        ),
        PredicateEntry::edb(
            "review_body_contains",
            &[Symbol, Symbol],
            keyed_on_needle,
            reviews(),
            "the review's body contains this needle, case-insensitively",
        ),
        PredicateEntry::edb(
            "comment",
            &[Symbol],
            enumerable,
            comments(),
            "a comment exists",
        ),
        PredicateEntry::edb(
            "comment_author",
            &[Symbol, Symbol],
            enumerable,
            comments(),
            "the comment's author",
        ),
        PredicateEntry::edb(
            "comment_subject",
            &[Symbol, Symbol],
            enumerable,
            comments(),
            "the `<kind>:<id>` the comment is attached to",
        ),
        PredicateEntry::edb(
            "comment_body_contains",
            &[Symbol, Symbol],
            keyed_on_needle,
            comments(),
            "the comment's body contains this needle, case-insensitively",
        ),
    ]
}

impl FactSource for ForgeFacts<'_> {
    fn provides(&self, key: &PredicateKey) -> bool {
        PROVIDED.contains(&(key.name.as_ref(), key.arity))
    }

    fn facts<'a>(
        &'a self,
        key: &PredicateKey,
        bound: Bindings<'_>,
        _attribute: bool,
    ) -> Result<Facts<'a>, HostError> {
        if !self.provides(key) {
            return Err(HostError::NoProvider(key.clone()));
        }

        let rows: Vec<Vec<Value>> = match (key.name.as_ref(), key.arity) {
            ("issue", 1) => all_issues(self.repo, key)?
                .into_iter()
                .map(|issue| vec![Value::sym(issue.id)])
                .collect(),
            ("issue_status", 2) => all_issues(self.repo, key)?
                .into_iter()
                .map(|issue| vec![Value::sym(issue.id), Value::sym(issue.status)])
                .collect(),
            ("issue_assignee", 2) => all_issues(self.repo, key)?
                .into_iter()
                .flat_map(|issue| {
                    issue
                        .assignees
                        .into_iter()
                        .map(move |name| vec![Value::sym(issue.id.clone()), Value::sym(name)])
                        .collect::<Vec<_>>()
                })
                .collect(),
            ("issue_reporter", 2) => all_issues(self.repo, key)?
                .into_iter()
                .flat_map(|issue| {
                    issue
                        .reporters
                        .into_iter()
                        .map(move |name| vec![Value::sym(issue.id.clone()), Value::sym(name)])
                        .collect::<Vec<_>>()
                })
                .collect(),
            ("issue_body_contains", 2) => {
                let needle = needle_arg(key, bound)?;
                all_issues(self.repo, key)?
                    .into_iter()
                    .filter(|issue| issue.body.to_ascii_lowercase().contains(&needle))
                    .map(|issue| vec![Value::sym(issue.id), Value::sym(needle.clone())])
                    .collect()
            }
            ("review", 1) => all_reviews(self.repo, key)?
                .into_iter()
                .map(|review| vec![Value::sym(review.id)])
                .collect(),
            ("review_status", 2) => all_reviews(self.repo, key)?
                .into_iter()
                .map(|review| vec![Value::sym(review.id), Value::sym(review.status)])
                .collect(),
            ("review_reviewer", 2) => all_reviews(self.repo, key)?
                .into_iter()
                .flat_map(|review| {
                    review
                        .reviewers
                        .into_iter()
                        .map(move |name| vec![Value::sym(review.id.clone()), Value::sym(name)])
                        .collect::<Vec<_>>()
                })
                .collect(),
            ("review_requester", 2) => all_reviews(self.repo, key)?
                .into_iter()
                .flat_map(|review| {
                    review
                        .requesters
                        .into_iter()
                        .map(move |name| vec![Value::sym(review.id.clone()), Value::sym(name)])
                        .collect::<Vec<_>>()
                })
                .collect(),
            ("review_target", 2) => all_reviews(self.repo, key)?
                .into_iter()
                .map(|review| vec![Value::sym(review.id), Value::sym(review.target.to_string())])
                .collect(),
            ("review_body_contains", 2) => {
                let needle = needle_arg(key, bound)?;
                all_reviews(self.repo, key)?
                    .into_iter()
                    .filter(|review| review.body.to_ascii_lowercase().contains(&needle))
                    .map(|review| vec![Value::sym(review.id), Value::sym(needle.clone())])
                    .collect()
            }
            ("comment", 1) => all_comments(self.repo, key)?
                .into_iter()
                .map(|comment| vec![Value::sym(comment.id)])
                .collect(),
            ("comment_author", 2) => all_comments(self.repo, key)?
                .into_iter()
                .map(|comment| vec![Value::sym(comment.id), Value::sym(comment.author)])
                .collect(),
            ("comment_subject", 2) => all_comments(self.repo, key)?
                .into_iter()
                .map(|comment| vec![Value::sym(comment.id), Value::sym(comment.subject)])
                .collect(),
            ("comment_body_contains", 2) => {
                let needle = needle_arg(key, bound)?;
                all_comments(self.repo, key)?
                    .into_iter()
                    .filter(|comment| comment.body.to_ascii_lowercase().contains(&needle))
                    .map(|comment| vec![Value::sym(comment.id), Value::sym(needle.clone())])
                    .collect()
            }
            _ => unreachable!("provides() already rejected any other key"),
        };

        let rows = apply_bound(rows, bound);
        Ok(Box::new(rows.into_iter().map(|row| Ok(Fact::new(row)))))
    }
}

/// Keep only rows agreeing with every bound position -- the generic
/// enumerable-EDB filter every simple predicate here needs on top of its own
/// row-building (`_body_contains` predicates already reflect their bound
/// needle in the row they emit, so this is a no-op on top of that).
fn apply_bound(rows: Vec<Vec<Value>>, bound: Bindings<'_>) -> Vec<Vec<Value>> {
    rows.into_iter()
        .filter(|row| {
            row.iter()
                .enumerate()
                .all(|(i, v)| bound.get(i).and_then(Option::as_ref).is_none_or(|b| b == v))
        })
        .collect()
}

/// The lowercased needle a `_body_contains` call's required-bound second
/// position supplies.
fn needle_arg(key: &PredicateKey, bound: Bindings<'_>) -> Result<String, HostError> {
    bound
        .get(1)
        .and_then(Option::as_ref)
        .and_then(Value::as_sym)
        .map(str::to_ascii_lowercase)
        .ok_or_else(|| HostError::UnsupportedMode { key: key.clone() })
}

fn backend(key: &PredicateKey, error: crate::error::Error) -> HostError {
    HostError::Backend {
        key: key.clone(),
        source: Box::new(error),
    }
}

fn all_issues(repo: &Repository, key: &PredicateKey) -> Result<Vec<Issue>, HostError> {
    let mut out = Vec::new();
    for id in Issue::list(repo).map_err(|e| backend(key, e))? {
        if let Some(issue) = Issue::load_from_repo(repo, &id).map_err(|e| backend(key, e))? {
            out.push(issue);
        }
    }
    Ok(out)
}

fn all_reviews(repo: &Repository, key: &PredicateKey) -> Result<Vec<Review>, HostError> {
    let mut out = Vec::new();
    for id in Review::list(repo).map_err(|e| backend(key, e))? {
        if let Some(review) = Review::load_from_repo(repo, &id).map_err(|e| backend(key, e))? {
            out.push(review);
        }
    }
    Ok(out)
}

fn all_comments(repo: &Repository, key: &PredicateKey) -> Result<Vec<Comment>, HostError> {
    let mut out = Vec::new();
    for id in Comment::list(repo).map_err(|e| backend(key, e))? {
        if let Some(comment) = Comment::load_from_repo(repo, &id).map_err(|e| backend(key, e))? {
            out.push(comment);
        }
    }
    Ok(out)
}

/// Quote `value` as a Datalog string literal (`gix-query-parse`'s lexer
/// escapes: `\\`, `\"`, `\n`, `\t`), so a caller-supplied name or keyword
/// embeds safely into goal text built with [`format!`].
#[must_use]
pub(crate) fn quote(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\0' => {}
            _ => out.push(ch),
        }
    }
    out.push('"');
    out
}

/// Run `goal` against `repo` through [`ForgeFacts`], layered over a fresh
/// `RepoFacts` by `gix_query::run_goal_with` -- the one path both `search`
/// and `query run --goal` take.
pub(crate) fn run_forge_goal(
    repo: &Repository,
    goal: &str,
    select: &[&str],
) -> Result<Vec<Vec<Value>>, QueryError> {
    let facts = ForgeFacts::new(repo);
    gix_query::run_goal_with(repo, &ForgeFacts::registry(), &facts, goal, select)
}

/// [`run_forge_goal`] for a bare predicate name, the path `query run
/// <predicate>` takes.
pub(crate) fn run_forge_predicate(
    repo: &Repository,
    predicate: &str,
    bound: &[(usize, Value)],
) -> Result<Vec<Vec<Value>>, QueryError> {
    let facts = ForgeFacts::new(repo);
    gix_query::run_predicate_with(repo, &ForgeFacts::registry(), &facts, predicate, bound)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::EntityOps;
    use crate::review::ReviewTarget;

    fn sample_repo() -> (tempfile::TempDir, Repository) {
        let dir = tempfile::tempdir().expect("tempdir");
        test_support::init_repo(dir.path());
        let repo = gix::open(dir.path()).expect("open repo");
        (dir, repo)
    }

    #[test]
    fn issue_assignee_predicate_answers_through_a_goal() {
        let (_dir, repo) = sample_repo();
        let issue = Issue {
            id: String::new(),
            status: "open".to_string(),
            title: "t".to_string(),
            body: "the frobnicator is broken".to_string(),
            labels: vec![],
            assignees: vec!["alice".to_string()],
            reporters: vec!["bob".to_string()],
            edit: None,
        };
        let id = issue.create_in_repo(&repo).expect("create issue");

        let rows = run_forge_goal(
            &repo,
            &format!("issue_assignee(Id, {})", quote("alice")),
            &["Id"],
        )
        .expect("run goal");
        assert_eq!(rows, vec![vec![Value::sym(id.as_str())]]);

        let none = run_forge_goal(
            &repo,
            &format!("issue_assignee(Id, {})", quote("nobody")),
            &["Id"],
        )
        .expect("run goal");
        assert!(none.is_empty());
    }

    #[test]
    fn issue_body_contains_is_case_insensitive_and_keyed_on_the_needle() {
        let (_dir, repo) = sample_repo();
        let issue = Issue {
            id: String::new(),
            status: "open".to_string(),
            title: "t".to_string(),
            body: "the Frobnicator is broken".to_string(),
            labels: vec![],
            assignees: vec![],
            reporters: vec![],
            edit: None,
        };
        let id = issue.create_in_repo(&repo).expect("create issue");

        let rows = run_forge_goal(
            &repo,
            &format!("issue_body_contains(Id, {})", quote("frobnicator")),
            &["Id"],
        )
        .expect("run goal");
        assert_eq!(rows, vec![vec![Value::sym(id.as_str())]]);
    }

    #[test]
    fn joins_a_forge_predicate_against_a_review_target() {
        let (_dir, repo) = sample_repo();
        let review = Review {
            id: String::new(),
            status: "open".to_string(),
            body: "please review".to_string(),
            reviewers: vec!["carol".to_string()],
            requesters: vec!["dave".to_string()],
            target: ReviewTarget::Commit {
                oid: gix::ObjectId::null(gix::hash::Kind::Sha1).to_string(),
            },
            edit: None,
        };
        let id = review.create_in_repo(&repo).expect("create review");

        let rows = run_forge_goal(
            &repo,
            &format!(
                "review_reviewer(Id, {}), review_target(Id, Target)",
                quote("carol")
            ),
            &["Id", "Target"],
        )
        .expect("run goal");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0][0], Value::sym(id.as_str()));
    }

    /// Validation is not bypassable: a predicate the registry declares but
    /// that neither `ForgeFacts` nor the layered-in `RepoFacts` answers for
    /// surfaces as an error, never as silent empty rows -- the same
    /// guarantee `gix-query`'s own `facts_seam.rs` proves for its `flagged`
    /// example, checked here against `ForgeFacts::registry` specifically.
    #[test]
    fn a_predicate_the_registry_declares_but_forge_facts_does_not_answer_is_an_error() {
        let (_dir, repo) = sample_repo();

        let mut registry = ForgeFacts::registry();
        registry
            .insert_predicate(PredicateEntry::edb(
                "unanswered",
                &[ValueType::Symbol],
                ArgSet::empty(),
                vec![Backing::AdHoc],
                "test-only: declared but implemented by no source",
            ))
            .expect("register unanswered/1");

        let facts = ForgeFacts::new(&repo);
        let err =
            gix_query::run_predicate_with(&repo, &registry, &facts, "unanswered", &[]).unwrap_err();
        assert!(
            matches!(err, QueryError::Eval(_)),
            "expected a host/eval error, got {err:?}"
        );
    }
}
