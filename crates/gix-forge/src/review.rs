//! [`Review`]: a forge doc requesting review of a target, plus
//! [`ReviewTarget`], the typed vocabulary a review request names.

use std::fmt;

use facet::Facet;

use crate::comment::Commentable;
use crate::entity::Entity;
use crate::error::Error;
use crate::{Authorization, Ownership};

#[derive(Debug, Clone, Facet)]
pub struct Review {
    pub id: String,
    pub status: String,
    pub body: String,
    pub reviewers: Vec<String>,
    pub requesters: Vec<String>,
    pub target: ReviewTarget,
    pub edit: Option<String>,
}

#[derive(Debug, Facet)]
pub struct StoredReview {
    status: String,
    body: String,
    reviewers: Vec<String>,
    requesters: Vec<String>,
    target: ReviewTarget,
    edit: Option<String>,
}

impl Entity for Review {
    const KIND: &'static str = "review";
    type Stored = StoredReview;

    fn id(&self) -> &str {
        &self.id
    }

    fn to_stored(&self) -> StoredReview {
        StoredReview {
            status: self.status.clone(),
            body: self.body.clone(),
            reviewers: self.reviewers.clone(),
            requesters: self.requesters.clone(),
            target: self.target.clone(),
            edit: self.edit.clone(),
        }
    }

    fn ownership(&self, authorization: &Authorization) -> Ownership {
        if let crate::Principal::Member(member) = authorization.principal()
            && self
                .requesters
                .iter()
                .any(|requester| requester == member.as_str())
        {
            Ownership::Owned
        } else {
            Ownership::NotOwned
        }
    }

    fn attribute_to(&mut self, principal: &str) {
        self.requesters = vec![principal.to_owned()];
    }

    fn from_stored(id: String, stored: StoredReview) -> Self {
        Self {
            id,
            status: stored.status,
            body: stored.body,
            reviewers: stored.reviewers,
            requesters: stored.requesters,
            target: stored.target,
            edit: stored.edit,
        }
    }
}

impl Commentable for Review {
    fn comment_subject(&self) -> (&'static str, &str) {
        (Review::KIND, &self.id)
    }
}

/// A target a review is attached to. Object ids are stored as their
/// hex-string form, since `gix::ObjectId` itself does not implement `Facet`;
/// parse with `gix::ObjectId::from_hex` to get a real id back.
#[derive(Debug, Clone, PartialEq, Eq, Facet)]
#[repr(u8)]
pub enum ReviewTarget {
    Blob { path: String, oid: String },
    Tree { oid: String },
    Commit { oid: String },
    BaseTipTreePair { base: String, tip: String },
    BaseTipCommitPair { base: String, tip: String },
    CommitRange { start: String, end: String },
}

impl ReviewTarget {
    /// Parse the `<kind>:<args>` shorthand the CLI accepts for `--target`,
    /// e.g. `commit:<oid>` or `blob:<path>:<oid>`. A bare, prefix-less value
    /// is treated as a commit oid.
    ///
    /// # Errors
    /// [`Error::InvalidTarget`] when a prefixed form is missing one of its
    /// required parts.
    pub fn parse(value: &str) -> Result<Self, Error> {
        let invalid = || Error::InvalidTarget(value.to_owned());

        if let Some(rest) = value.strip_prefix("commit:") {
            if rest.is_empty() {
                return Err(invalid());
            }
            return Ok(ReviewTarget::Commit {
                oid: rest.to_owned(),
            });
        }
        if let Some(rest) = value.strip_prefix("tree:") {
            if rest.is_empty() {
                return Err(invalid());
            }
            return Ok(ReviewTarget::Tree {
                oid: rest.to_owned(),
            });
        }
        if let Some(rest) = value.strip_prefix("blob:") {
            let (path, oid) = split_two(rest).ok_or_else(invalid)?;
            return Ok(ReviewTarget::Blob { path, oid });
        }
        if let Some(rest) = value.strip_prefix("base-tip-tree:") {
            let (base, tip) = split_two(rest).ok_or_else(invalid)?;
            return Ok(ReviewTarget::BaseTipTreePair { base, tip });
        }
        if let Some(rest) = value.strip_prefix("base-tip-commit:") {
            let (base, tip) = split_two(rest).ok_or_else(invalid)?;
            return Ok(ReviewTarget::BaseTipCommitPair { base, tip });
        }
        if let Some(rest) = value.strip_prefix("commit-range:") {
            let (start, end) = split_two(rest).ok_or_else(invalid)?;
            return Ok(ReviewTarget::CommitRange { start, end });
        }
        if value.is_empty() {
            return Err(invalid());
        }
        Ok(ReviewTarget::Commit {
            oid: value.to_owned(),
        })
    }
}

/// Split `rest` on its first `:`, rejecting an empty half on either side.
fn split_two(rest: &str) -> Option<(String, String)> {
    let (a, b) = rest.split_once(':')?;
    if a.is_empty() || b.is_empty() {
        return None;
    }
    Some((a.to_owned(), b.to_owned()))
}

impl fmt::Display for ReviewTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReviewTarget::Blob { path, oid } => write!(f, "blob:{path}:{oid}"),
            ReviewTarget::Tree { oid } => write!(f, "tree:{oid}"),
            ReviewTarget::Commit { oid } => write!(f, "commit:{oid}"),
            ReviewTarget::BaseTipTreePair { base, tip } => {
                write!(f, "base-tip-tree:{base}:{tip}")
            }
            ReviewTarget::BaseTipCommitPair { base, tip } => {
                write!(f, "base-tip-commit:{base}:{tip}")
            }
            ReviewTarget::CommitRange { start, end } => {
                write!(f, "commit-range:{start}:{end}")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::EntityOps;
    use crate::{Authorization, Principal, open_store};

    fn auth() -> Authorization {
        Authorization::new(Principal::member_id("alice"))
    }

    #[test]
    fn review_round_trip_through_store() {
        let dir = tempfile::tempdir().expect("tempdir");
        test_support::init_repo(dir.path());
        let repo = gix::open(dir.path()).expect("open repo");
        let store = open_store(&repo);

        Review::ensure_schema_as(&store, &auth()).expect("publish review schema");

        let review = Review {
            id: "review-1".to_string(),
            status: "open".to_string(),
            body: "round trip review".to_string(),
            reviewers: vec!["carol".to_string()],
            requesters: vec!["alice".to_string()],
            target: ReviewTarget::CommitRange {
                start: gix::ObjectId::null(gix::hash::Kind::Sha1).to_string(),
                end: gix::ObjectId::null(gix::hash::Kind::Sha1).to_string(),
            },
            edit: Some("initial edit note".to_string()),
        };

        review.save_as(&store, &auth()).expect("save review");
        let loaded = Review::load(&store, &review.id)
            .expect("load review")
            .expect("review exists");

        assert_eq!(loaded.id, review.id);
        assert_eq!(loaded.status, review.status);
        assert_eq!(loaded.body, review.body);
        assert_eq!(loaded.reviewers, review.reviewers);
        assert_eq!(loaded.requesters, review.requesters);
        assert_eq!(loaded.edit, review.edit);
        assert_eq!(loaded.target, review.target);
    }

    #[test]
    fn creation_attributes_the_requester_to_the_principal() {
        let dir = tempfile::tempdir().expect("tempdir");
        test_support::init_repo(dir.path());
        let repo = gix::open(dir.path()).expect("open repo");
        let review = Review {
            id: String::new(),
            status: "open".to_owned(),
            body: "review body".to_owned(),
            reviewers: vec![],
            requesters: vec!["spoofed".to_owned()],
            target: ReviewTarget::Commit {
                oid: "deadbeef".to_owned(),
            },
            edit: None,
        };

        let id = review
            .create_in_repo_as(&repo, &auth())
            .expect("create review");
        let loaded = Review::load_from_repo(&repo, &id)
            .expect("load review")
            .expect("review exists");
        assert_eq!(loaded.requesters, vec!["alice"]);
    }

    #[test]
    fn target_parse_round_trips_through_display() {
        let cases = [
            "commit:abc",
            "tree:abc",
            "blob:src/lib.rs:abc",
            "base-tip-tree:a:b",
            "base-tip-commit:a:b",
            "commit-range:a:b",
        ];
        for case in cases {
            let target = ReviewTarget::parse(case).unwrap_or_else(|e| panic!("{case}: {e}"));
            assert_eq!(target.to_string(), case);
        }
        // A bare value with no recognized prefix is a commit oid.
        assert_eq!(
            ReviewTarget::parse("deadbeef").unwrap(),
            ReviewTarget::Commit {
                oid: "deadbeef".to_owned()
            }
        );
    }

    #[test]
    fn target_parse_rejects_a_malformed_prefixed_form() {
        assert!(ReviewTarget::parse("blob:onlypath").is_err());
        assert!(ReviewTarget::parse("blob::").is_err());
    }
}
