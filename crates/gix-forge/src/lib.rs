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

mod authorization;
mod comment;
mod entity;
mod error;
mod facts;
mod issue;
mod member;
mod review;
mod search;
mod status;

use gix::{ObjectId, Repository};
use gix_store::{Layout, RefPrefix, RefSegment, RepoStore};

pub use authorization::{Authorization, Capability, MemberId, Ownership, Principal};
pub use comment::{Comment, Commentable, binding_genesis};
pub use entity::{Entity, EntityOps};
pub use error::Error;
pub use facts::ForgeFacts;
pub use gix_anchor::{Binding, LineRange};
pub use gix_query::Value as QueryValue;
pub use issue::Issue;
pub use member::Member;
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
pub use member::StoredMember;
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
/// This legacy entry point fails closed; use [`ensure_issue_schema_as`] for an
/// authenticated write.
///
/// # Errors
/// See [`Error`].
pub fn ensure_issue_schema(repo: &Repository) -> Result<ObjectId, Error> {
    Issue::ensure_schema(&open_store(repo))
}

/// Publish (or evolve) the `issue` schema after authorization.
///
/// # Errors
/// See [`Error`].
pub fn ensure_issue_schema_as(
    repo: &Repository,
    authorization: &Authorization,
) -> Result<ObjectId, Error> {
    Issue::ensure_schema_as(&open_store(repo), authorization)
}

/// Publish (or evolve) the `review` schema.
///
/// This legacy entry point fails closed; use [`ensure_review_schema_as`] for an
/// authenticated write.
///
/// # Errors
/// See [`Error`].
pub fn ensure_review_schema(repo: &Repository) -> Result<ObjectId, Error> {
    Review::ensure_schema(&open_store(repo))
}

/// Publish (or evolve) the `review` schema after authorization.
///
/// # Errors
/// See [`Error`].
pub fn ensure_review_schema_as(
    repo: &Repository,
    authorization: &Authorization,
) -> Result<ObjectId, Error> {
    Review::ensure_schema_as(&open_store(repo), authorization)
}

/// Publish (or evolve) the `member` schema.
///
/// This legacy entry point fails closed; use [`ensure_member_schema_as`] for an
/// authenticated write.
///
/// # Errors
/// See [`Error`].
pub fn ensure_member_schema(repo: &Repository) -> Result<ObjectId, Error> {
    Member::ensure_schema(&open_store(repo))
}

/// Publish (or evolve) the `member` schema after authorization.
///
/// # Errors
/// See [`Error`].
pub fn ensure_member_schema_as(
    repo: &Repository,
    authorization: &Authorization,
) -> Result<ObjectId, Error> {
    Member::ensure_schema_as(&open_store(repo), authorization)
}

/// Publish (or evolve) the `comment` schema.
///
/// This legacy entry point fails closed; use [`ensure_comment_schema_as`] for
/// an authenticated write.
///
/// # Errors
/// See [`Error`].
pub fn ensure_comment_schema(repo: &Repository) -> Result<ObjectId, Error> {
    Comment::ensure_schema(&open_store(repo))
}

/// Publish (or evolve) the `comment` schema after authorization.
///
/// # Errors
/// See [`Error`].
pub fn ensure_comment_schema_as(
    repo: &Repository,
    authorization: &Authorization,
) -> Result<ObjectId, Error> {
    Comment::ensure_schema_as(&open_store(repo), authorization)
}

/// Install the built-in `review` rule module and validate the whole program
/// -- including every predicate [`ForgeFacts`] declares -- against it.
///
/// This legacy entry point fails closed; use [`install_builtin_query_rules_as`]
/// for an authenticated write.
///
/// # Errors
/// See [`Error`].
pub fn install_builtin_query_rules(_repo: &Repository) -> Result<(), Error> {
    Err(Error::Unauthorized {
        capability: Capability::ForgeInstall,
    })
}

/// Install the built-in query rules after authorization.
///
/// # Errors
/// See [`Error`].
pub fn install_builtin_query_rules_as(
    repo: &Repository,
    authorization: &Authorization,
) -> Result<(), Error> {
    authorization.check(Capability::ForgeInstall, Ownership::NotApplicable)?;
    let store = gix_query::RuleStore::open(repo).map_err(|e| Error::QueryRules(e.to_string()))?;
    store
        .put("review", BUILTIN_REVIEW_RULES)
        .map_err(|e| Error::QueryRules(e.to_string()))?;
    gix_query::checked_program_with(repo, &ForgeFacts::registry())?;
    Ok(())
}

/// Remove forge schemas and the built-in query rules from an empty forge.
///
/// # Errors
/// Returns an error if any forge entity remains or a reference cannot be removed.
pub fn uninstall(repo: &Repository) -> Result<(), Error> {
    let store = open_store(repo);
    for kind in [Issue::KIND, Review::KIND, Member::KIND, Comment::KIND] {
        let kind = RefSegment::new(kind).expect("built-in kind is valid");
        let kind_name = kind.to_string();
        if !store.dynamic(kind).list()?.is_empty() {
            return Err(Error::DataPresent(kind_name));
        }
    }

    let rules = gix_query::RuleStore::open(repo).map_err(|e| Error::QueryRules(e.to_string()))?;
    let has_review_rule = rules
        .get("review")
        .map_err(|e| Error::QueryRules(e.to_string()))?
        .is_some();
    if has_review_rule {
        rules
            .delete("review")
            .map_err(|e| Error::QueryRules(e.to_string()))?;
    }

    for kind in [
        Issue::KIND,
        Review::KIND,
        Member::KIND,
        Comment::KIND,
        "rules",
    ] {
        delete_reference_if_present(repo, &format!("refs/schema/{kind}"))?;
    }
    Ok(())
}

fn delete_reference_if_present(repo: &Repository, name: &str) -> Result<(), Error> {
    let Some(reference) = repo
        .try_find_reference(name)
        .map_err(|e| Error::Uninstall(e.to_string()))?
    else {
        return Ok(());
    };
    reference
        .delete()
        .map_err(|e| Error::Uninstall(e.to_string()))
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
