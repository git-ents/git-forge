//! Core logic for git-forge.

use facet::Facet;
use gix::{ObjectId, Repository};
use gix_comment::{Binding, Comments};

pub use gix_comment::{Comment, State as CommentState};
pub use gix_query::Value as QueryValue;

/// Errors from `gix-forge`'s storage operations.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Failed at the `gix-store` layer (missing schema, git error, etc).
    #[error(transparent)]
    Store(#[from] gix_store::Error),
    /// Failed converting a typed value into the dynamic `Value` `gix-store` stores.
    #[error("failed to convert to a storage value: {0}")]
    ToValue(String),
    /// Failed converting a stored `Value` back into a typed value.
    #[error(transparent)]
    FromValue(#[from] facet_value::ValueError),
    /// Failed to derive a `SchemaDoc` for a type.
    #[error("failed to derive schema: {0}")]
    Schema(String),
    /// Failed at the `gix-query` layer.
    #[error(transparent)]
    Query(#[from] gix_query::QueryError),
    /// Failed storing built-in query rules.
    #[error("failed to install built-in query rules: {0}")]
    QueryRules(String),
}

// =========================================================================
// 1. Entities & Enums
// =========================================================================

#[derive(Debug, Facet)]
pub struct Issue {
    pub id: String,
    pub title: String,
    pub body: String,
    pub labels: Vec<String>,
    pub assignees: Vec<String>,
    pub reporters: Vec<String>,
    pub edit: Option<String>,
}

impl Issue {
    /// The `gix-store` kind this entity is published under.
    pub const KIND: &'static str = "issue";

    /// Publish (or evolve) the `issue` schema in `store`. Call this once
    /// before the first `save`.
    pub fn ensure_schema(store: &gix_store::Store<'_>) -> Result<ObjectId, Error> {
        let doc = gix_store::schema_of::<Issue>().map_err(|e| Error::Schema(e.to_string()))?;
        Ok(store.put_schema(Self::KIND, &doc)?)
    }

    /// Store this issue at `refs/store/issue/<id>`.
    pub fn save(&self, store: &gix_store::Store<'_>) -> Result<ObjectId, Error> {
        let value = facet_value::to_value(self).map_err(|e| Error::ToValue(e.to_string()))?;
        Ok(store.store(Self::KIND, &self.id, &value, None)?)
    }

    /// Load the issue named `id`, or `None` if it doesn't exist.
    pub fn load(store: &gix_store::Store<'_>, id: &str) -> Result<Option<Issue>, Error> {
        let Some(value) = store.retrieve(Self::KIND, id)? else {
            return Ok(None);
        };
        Ok(Some(facet_value::from_value(value)?))
    }

    /// Ensure schema and save to the repository-backed store.
    pub fn save_in_repo(&self, repo: &Repository) -> Result<ObjectId, Error> {
        let store = gix_store::Store::open(repo);
        Self::ensure_schema(&store)?;
        self.save(&store)
    }

    /// Load an issue from the repository-backed store.
    pub fn load_from_repo(repo: &Repository, id: &str) -> Result<Option<Issue>, Error> {
        let store = gix_store::Store::open(repo);
        Self::load(&store, id)
    }

    /// List issue ids in the repository-backed store.
    pub fn list(repo: &Repository) -> Result<Vec<String>, Error> {
        let store = gix_store::Store::open(repo);
        Ok(store.list(Self::KIND)?)
    }

    /// List issue version history, newest first.
    pub fn history(repo: &Repository, id: &str) -> Result<Vec<ObjectId>, Error> {
        let store = gix_store::Store::open(repo);
        Ok(store.history(Self::KIND, id)?)
    }

    /// Delete an issue by id.
    pub fn delete(repo: &Repository, id: &str) -> Result<bool, Error> {
        let store = gix_store::Store::open(repo);
        Ok(store.delete(Self::KIND, id)?)
    }
}

#[derive(Debug, Facet)]
pub struct Review {
    pub id: String,
    pub body: String,
    pub reviewers: Vec<String>,
    pub requesters: Vec<String>,
    pub target: ReviewTarget,
    pub edit: Option<String>,
}

pub fn ensure_issue_schema(repo: &Repository) -> Result<ObjectId, Error> {
    let store = gix_store::Store::open(repo);
    Issue::ensure_schema(&store)
}

pub fn ensure_review_schema(repo: &Repository) -> Result<ObjectId, Error> {
    let store = gix_store::Store::open(repo);
    Review::ensure_schema(&store)
}

pub fn install_builtin_query_rules(repo: &Repository) -> Result<(), Error> {
    let store = gix_query::RuleStore::open(repo).map_err(|e| Error::QueryRules(e.to_string()))?;
    store
        .put("review", BUILTIN_REVIEW_RULES)
        .map_err(|e| Error::QueryRules(e.to_string()))?;
    gix_query::checked_program(repo)?;
    Ok(())
}

pub fn query_goal(
    repo: &Repository,
    goal: &str,
    select: &[&str],
) -> Result<Vec<Vec<QueryValue>>, Error> {
    Ok(gix_query::run_goal(repo, goal, select)?)
}

pub fn query_predicate(
    repo: &Repository,
    predicate: &str,
    bound: &[(usize, QueryValue)],
) -> Result<Vec<Vec<QueryValue>>, Error> {
    Ok(gix_query::run_predicate(repo, predicate, bound)?)
}

const BUILTIN_REVIEW_RULES: &str = r#"
pub reviewed(B).
pub unreviewed(Rev, B).
pub blocked(Rev).
pub mergeable(Rev).

active_member(M) :- member(M), !revoked(M).
review(C)        :- claim(C), kind(C, review).

approved_by(B, M) :- review(C), target(C, B), signer(C, M),
                     verdict(C, approve), active_member(M).
rejected(B)       :- review(C), target(C, B), verdict(C, reject),
                     signer(C, M), active_member(M).
reviewed(B)       :- approved_by(B, _).

reach(Rev, Rev) :- commit(Rev).
reach(Rev, C)   :- reach(Rev, X), parent(X, C).

has_parent(C)         :- parent(C, _).
introduced(Rev, B, C) :- reach(Rev, C), tree_entry(C, P, B),
                         parent(C, Pc), !tree_entry(Pc, P, B).
introduced(Rev, B, C) :- reach(Rev, C), tree_entry(C, P, B), !has_parent(C).
authored(Rev, B, M)   :- introduced(Rev, B, C), author(C, M).
self_approved(Rev, B, M) :- authored(Rev, B, M), approved_by(B, M).

unreviewed(Rev, B) :- tree_entry(Rev, _, B), !reviewed(B).
blocked(Rev)       :- unreviewed(Rev, _).
blocked(Rev)       :- tree_entry(Rev, _, B), rejected(B).
blocked(Rev)       :- tree_entry(Rev, _, B), self_approved(Rev, B, _).
mergeable(Rev)     :- commit(Rev), !blocked(Rev).
"#;

impl Review {
    /// The `gix-store` kind this entity is published under.
    pub const KIND: &'static str = "review";

    /// Publish (or evolve) the `review` schema in `store`. Call this once
    /// before the first `save`.
    pub fn ensure_schema(store: &gix_store::Store<'_>) -> Result<ObjectId, Error> {
        let doc = gix_store::schema_of::<Review>().map_err(|e| Error::Schema(e.to_string()))?;
        Ok(store.put_schema(Self::KIND, &doc)?)
    }

    /// Store this review at `refs/store/review/<id>`.
    pub fn save(&self, store: &gix_store::Store<'_>) -> Result<ObjectId, Error> {
        let value = facet_value::to_value(self).map_err(|e| Error::ToValue(e.to_string()))?;
        Ok(store.store(Self::KIND, &self.id, &value, None)?)
    }

    /// Load the review named `id`, or `None` if it doesn't exist.
    pub fn load(store: &gix_store::Store<'_>, id: &str) -> Result<Option<Review>, Error> {
        let Some(value) = store.retrieve(Self::KIND, id)? else {
            return Ok(None);
        };
        Ok(Some(facet_value::from_value(value)?))
    }

    /// Ensure schema and save to the repository-backed store.
    pub fn save_in_repo(&self, repo: &Repository) -> Result<ObjectId, Error> {
        let store = gix_store::Store::open(repo);
        Self::ensure_schema(&store)?;
        self.save(&store)
    }

    /// Load a review from the repository-backed store.
    pub fn load_from_repo(repo: &Repository, id: &str) -> Result<Option<Review>, Error> {
        let store = gix_store::Store::open(repo);
        Self::load(&store, id)
    }

    /// List review ids in the repository-backed store.
    pub fn list(repo: &Repository) -> Result<Vec<String>, Error> {
        let store = gix_store::Store::open(repo);
        Ok(store.list(Self::KIND)?)
    }

    /// List review version history, newest first.
    pub fn history(repo: &Repository, id: &str) -> Result<Vec<ObjectId>, Error> {
        let store = gix_store::Store::open(repo);
        Ok(store.history(Self::KIND, id)?)
    }

    /// Delete a review by id.
    pub fn delete(repo: &Repository, id: &str) -> Result<bool, Error> {
        let store = gix_store::Store::open(repo);
        Ok(store.delete(Self::KIND, id)?)
    }
}

/// A target a review is attached to. Object ids are stored as their
/// hex-string form, since `gix::ObjectId` itself does not implement `Facet`;
/// parse with `gix::ObjectId::from_hex` to get a real id back.
#[derive(Debug, Clone, Facet)]
#[repr(u8)]
pub enum ReviewTarget {
    Blob { path: String, oid: String },
    Tree { oid: String },
    Commit { oid: String },
    BaseTipTreePair { base: String, tip: String },
    BaseTipCommitPair { base: String, tip: String },
    CommitRange { start: String, end: String },
}

// =========================================================================
// 2. Sugar for Comments (Attachment API)
// =========================================================================

#[derive(Debug, Clone)]
pub struct Anchor {
    pub target: Target,
    pub position: Position,
    pub comment: Comment,
    pub edit: Option<String>,
}

#[derive(Debug, Clone)]
pub enum Target {
    Blob(String),
    Tree,
}

#[derive(Debug, Clone)]
pub enum Position {
    LineRange(std::ops::Range<usize>),
}

pub trait Commentable {
    /// Returns the stable Git reference path under which comments/anchors are stored.
    fn comments_ref(&self) -> String;

    /// Attaches an unstructured or global thread comment to this entity's tree OID.
    fn add_comment(
        &self,
        _repo: &gix::Repository,
        _comment: Comment,
    ) -> Result<ObjectId, Box<dyn std::error::Error>> {
        let _comment_ref = self.comments_ref();
        // TODO: implement comment storage
        Ok(ObjectId::null(gix::hash::Kind::Sha1))
    }

    /// Attaches an anchored/inline comment to a specific position (e.g. line range) inside the entity's target.
    fn add_anchored_comment(
        &self,
        _repo: &gix::Repository,
        target: Target,
        position: Position,
        comment: Comment,
    ) -> Result<ObjectId, Box<dyn std::error::Error>> {
        let _anchor = Anchor {
            target,
            position,
            comment,
            edit: None,
        };
        let _comments_ref = self.comments_ref();
        // TODO: implement anchored comment storage
        Ok(ObjectId::null(gix::hash::Kind::Sha1))
    }

    /// Retrieves all comments (both threaded and inline/anchored) associated with this entity.
    fn get_comments(
        &self,
        repo: &gix::Repository,
    ) -> Result<Vec<Anchor>, Box<dyn std::error::Error>> {
        let comments = Comments::open(repo).list(None)?;

        let mut anchors = Vec::with_capacity(comments.len());
        for comment in comments {
            let (target, position) = match &comment.binding {
                Binding::Position(anchor) => {
                    let range = match anchor.lines {
                        Some(lines) => {
                            let start = usize::try_from(lines.start)?;
                            let end_exclusive = usize::try_from(lines.end.saturating_add(1))?;
                            start..end_exclusive.max(start)
                        }
                        None => 0..0,
                    };
                    (
                        Target::Blob(anchor.path.clone()),
                        Position::LineRange(range),
                    )
                }
                _ => (Target::Tree, Position::LineRange(0..0)),
            };

            anchors.push(Anchor {
                target,
                position,
                comment,
                edit: None,
            });
        }

        Ok(anchors)
    }
}

impl Commentable for Issue {
    fn comments_ref(&self) -> String {
        format!("refs/forge/issues/{}/comments", self.id)
    }
}

impl Commentable for Review {
    fn comments_ref(&self) -> String {
        format!("refs/forge/reviews/{}/comments", self.id)
    }
}

// =========================================================================
// 3. Example Usage
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn example_issue_creation() {
        let _issue = Issue {
            id: "123".to_string(),
            title: "Cannot save issue".to_string(),
            body: "This is a bug report.".to_string(),
            labels: vec!["bug".to_string(), "high-priority".to_string()],
            assignees: vec!["jdc-pub".to_string()],
            reporters: vec!["alice".to_string()],
            edit: None,
        };
    }

    #[test]
    fn example_review_creation() {
        let _review = Review {
            id: "456".to_string(),
            body: "Please review the changes in this range.".to_string(),
            reviewers: vec!["bob".to_string()],
            requesters: vec!["alice".to_string()],
            target: ReviewTarget::CommitRange {
                start: ObjectId::null(gix::hash::Kind::Sha1).to_string(),
                end: ObjectId::null(gix::hash::Kind::Sha1).to_string(),
            },
            edit: None,
        };
    }

    #[test]
    fn issue_round_trip_through_store() {
        let dir = tempfile::tempdir().expect("tempdir");
        test_support::init_repo(dir.path());
        let repo = gix::open(dir.path()).expect("open repo");
        let store = gix_store::Store::open(&repo);

        Issue::ensure_schema(&store).expect("publish issue schema");

        let issue = Issue {
            id: "issue-1".to_string(),
            title: "Round trip issue".to_string(),
            body: "round trip issue".to_string(),
            labels: vec!["bug".to_string(), "P1".to_string()],
            assignees: vec!["alice".to_string()],
            reporters: vec!["bob".to_string()],
            edit: Some("initial edit note".to_string()),
        };

        issue.save(&store).expect("save issue");
        let loaded = Issue::load(&store, &issue.id)
            .expect("load issue")
            .expect("issue exists");

        assert_eq!(loaded.id, issue.id);
        assert_eq!(loaded.title, issue.title);
        assert_eq!(loaded.body, issue.body);
        assert_eq!(loaded.labels, issue.labels);
        assert_eq!(loaded.assignees, issue.assignees);
        assert_eq!(loaded.reporters, issue.reporters);
        assert_eq!(loaded.edit, issue.edit);
    }

    #[test]
    fn review_round_trip_through_store() {
        let dir = tempfile::tempdir().expect("tempdir");
        test_support::init_repo(dir.path());
        let repo = gix::open(dir.path()).expect("open repo");
        let store = gix_store::Store::open(&repo);

        Review::ensure_schema(&store).expect("publish review schema");

        let review = Review {
            id: "review-1".to_string(),
            body: "round trip review".to_string(),
            reviewers: vec!["carol".to_string()],
            requesters: vec!["dave".to_string()],
            target: ReviewTarget::CommitRange {
                start: ObjectId::null(gix::hash::Kind::Sha1).to_string(),
                end: ObjectId::null(gix::hash::Kind::Sha1).to_string(),
            },
            edit: Some("initial edit note".to_string()),
        };

        review.save(&store).expect("save review");
        let loaded = Review::load(&store, &review.id)
            .expect("load review")
            .expect("review exists");

        assert_eq!(loaded.id, review.id);
        assert_eq!(loaded.body, review.body);
        assert_eq!(loaded.reviewers, review.reviewers);
        assert_eq!(loaded.requesters, review.requesters);
        assert_eq!(loaded.edit, review.edit);
        match (&loaded.target, &review.target) {
            (
                ReviewTarget::CommitRange {
                    start: loaded_start,
                    end: loaded_end,
                },
                ReviewTarget::CommitRange {
                    start: expected_start,
                    end: expected_end,
                },
            ) => {
                assert_eq!(loaded_start, expected_start);
                assert_eq!(loaded_end, expected_end);
            }
            _ => panic!("unexpected target variant"),
        }
    }
}
