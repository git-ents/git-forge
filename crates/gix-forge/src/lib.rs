//! Core logic for `git-forge`.
//!
//! Per ARCHITECTURE.md, "git-forge... composes everything, owns no primitive
//! logic": [`entity`] is the one CRUD implementation every entity kind
//! (`Issue`, `Review`, `Comment`) shares over `gix-store`; [`Comment`] embeds
//! a `gix_anchor::Binding` subtree inline rather than reimplementing bind
//! resolution ("Forge reads binds through query only"); [`facts::ForgeFacts`]
//! is the only place forge data is read to answer a query, and [`search`]
//! compiles to the same Datalog goals `query` runs -- never an in-memory
//! scan.

mod comment;
mod entity;
mod error;
mod facts;
mod issue;
mod review;
mod search;
mod status;

use gix::{ObjectId, Repository};
use gix_store::{Layout, RefPrefix, RepoStore};

pub use comment::{Comment, Commentable, binding_genesis};
pub use entity::{Entity, EntityOps};
pub use error::Error;
pub use facts::ForgeFacts;
pub use gix_anchor::{Binding, LineRange};
pub use gix_query::Value as QueryValue;
pub use issue::Issue;
pub use review::{Review, ReviewTarget};
pub use search::{
    HitKind, SearchHit, search_assignee, search_comment, search_find, search_issue, search_keyword,
    search_requester, search_review, search_reviewer,
};
pub use status::Status;

#[doc(hidden)]
pub use comment::StoredComment;
#[doc(hidden)]
pub use issue::StoredIssue;
#[doc(hidden)]
pub use review::StoredReview;

fn layout() -> Layout {
    Layout {
        data: RefPrefix::new("refs/forge").expect("built-in ref prefix is valid"),
        schema: RefPrefix::new("refs/schema").expect("built-in ref prefix is valid"),
    }
}

fn open_store(repo: &Repository) -> RepoStore<'_> {
    RepoStore::open_with_layout(repo, layout())
}

/// Publish (or evolve) the `issue` schema.
///
/// # Errors
/// See [`Error`].
pub fn ensure_issue_schema(repo: &Repository) -> Result<ObjectId, Error> {
    Issue::ensure_schema(&open_store(repo))
}

/// Publish (or evolve) the `review` schema.
///
/// # Errors
/// See [`Error`].
pub fn ensure_review_schema(repo: &Repository) -> Result<ObjectId, Error> {
    Review::ensure_schema(&open_store(repo))
}

/// Publish (or evolve) the `comment` schema.
///
/// # Errors
/// See [`Error`].
pub fn ensure_comment_schema(repo: &Repository) -> Result<ObjectId, Error> {
    Comment::ensure_schema(&open_store(repo))
}

/// Install the built-in `review` rule module and validate the whole program
/// -- including every predicate [`ForgeFacts`] declares -- against it.
///
/// # Errors
/// See [`Error`].
pub fn install_builtin_query_rules(repo: &Repository) -> Result<(), Error> {
    let store = gix_query::RuleStore::open(repo).map_err(|e| Error::QueryRules(e.to_string()))?;
    store
        .put("review", BUILTIN_REVIEW_RULES)
        .map_err(|e| Error::QueryRules(e.to_string()))?;
    gix_query::checked_program_with(repo, &ForgeFacts::registry())?;
    Ok(())
}

/// Run `goal`, selecting `select`'s columns, against every host predicate
/// plus every predicate [`ForgeFacts`] declares.
///
/// # Errors
/// See [`Error`].
pub fn query_goal(
    repo: &Repository,
    goal: &str,
    select: &[&str],
) -> Result<Vec<Vec<QueryValue>>, Error> {
    Ok(facts::run_forge_goal(repo, goal, select)?)
}

/// [`query_goal`] for a bare predicate name, in the tier-1 `--bind` style.
///
/// # Errors
/// See [`Error`].
pub fn query_predicate(
    repo: &Repository,
    predicate: &str,
    bound: &[(usize, QueryValue)],
) -> Result<Vec<Vec<QueryValue>>, Error> {
    Ok(facts::run_forge_predicate(repo, predicate, bound)?)
}

const BUILTIN_REVIEW_RULES: &str = r#"
pub reviewed(B).
pub unreviewed(Rev, B).
pub blocked(Rev).
pub mergeable(Rev).

active_member(M)  :- member(M), !revoked(M).
review_claim(C)   :- claim(C), kind(C, review).

approved_by(B, M) :- review_claim(C), target(C, B), signer(C, M),
                     verdict(C, approve), active_member(M).
rejected(B)       :- review_claim(C), target(C, B), verdict(C, reject),
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
