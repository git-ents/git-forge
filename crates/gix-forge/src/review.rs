//! [`Review`]: a forge doc requesting review of a target, plus
//! [`ReviewTarget`], the typed vocabulary a review request names.

use std::collections::BTreeMap;
use std::fmt;

use facet::Facet;
use facet_git_tree::normal_form::{self, NormalForm};
use gix::ObjectId;

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
            target: self.target.canonicalized(),
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

/// A validated, exact Git object id.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Facet)]
pub struct ExactOid(String);

impl ExactOid {
    /// Parse a canonical SHA-1 object id.
    pub fn parse(value: &str) -> Result<Self, Error> {
        ObjectId::from_hex(value.as_bytes())
            .map(|oid| Self(oid.to_string()))
            .map_err(|_| Error::InvalidTarget(format!("malformed Git object id `{value}`")))
    }

    /// Return the Git object id represented by this value.
    pub fn oid(&self) -> ObjectId {
        ObjectId::from_hex(self.0.as_bytes()).expect("ExactOid invariant")
    }
}

impl fmt::Display for ExactOid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A blob target. The path is descriptive and is deliberately excluded from identity.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Facet)]
pub struct PinnedBlob {
    pub path: String,
    pub oid: ExactOid,
}

/// A tree target retained by its exact tree object id.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Facet)]
pub struct PinnedTree {
    pub oid: ExactOid,
}

/// The five canonical shapes of material a review can target.
#[derive(Debug, Clone, PartialEq, Eq, Facet)]
#[repr(u8)]
pub enum ReviewTarget {
    Blobs(Vec<PinnedBlob>),
    Trees(Vec<PinnedTree>),
    TreeRange { base: PinnedTree, tip: PinnedTree },
    Commits(Vec<ExactOid>),
    CommitRange { base: ExactOid, tip: ExactOid },
}

impl ReviewTarget {
    /// Parse the CLI shorthand. Collections use comma-separated values.
    pub fn parse(value: &str) -> Result<Self, Error> {
        let invalid = || Error::InvalidTarget(value.to_owned());
        let (kind, rest) = value.split_once(':').unwrap_or(("commit", value));
        match kind {
            "blob" => {
                let (path, oid) = rest.rsplit_once(':').ok_or_else(invalid)?;
                Ok(Self::Blobs(vec![PinnedBlob {
                    path: path.into(),
                    oid: ExactOid::parse(oid)?,
                }]))
            }
            "tree" => Ok(Self::Trees(vec![PinnedTree {
                oid: ExactOid::parse(rest)?,
            }])),
            "commit" => Ok(Self::Commits(vec![ExactOid::parse(rest)?])),
            "base-tip-tree" | "tree-range" => {
                let (base, tip) = split_two(rest).ok_or_else(invalid)?;
                Ok(Self::TreeRange {
                    base: PinnedTree {
                        oid: ExactOid::parse(base)?,
                    },
                    tip: PinnedTree {
                        oid: ExactOid::parse(tip)?,
                    },
                })
            }
            "base-tip-commit" | "commit-range" => {
                let (base, tip) = split_two(rest).ok_or_else(invalid)?;
                Ok(Self::CommitRange {
                    base: ExactOid::parse(base)?,
                    tip: ExactOid::parse(tip)?,
                })
            }
            "blobs" => parse_blobs(rest),
            "trees" => parse_list(rest, |oids| {
                Self::Trees(oids.into_iter().map(|oid| PinnedTree { oid }).collect())
            }),
            "commits" => parse_list(rest, Self::Commits),
            _ => Err(invalid()),
        }
    }

    /// Return a canonical copy with unordered members sorted by OID.
    pub fn canonicalized(&self) -> Self {
        match self {
            Self::Blobs(items) => {
                let mut items = items.clone();
                items.sort_by(|a, b| a.oid.cmp(&b.oid));
                Self::Blobs(items)
            }
            Self::Trees(items) => Self::Trees(canonicalize(
                items
                    .clone()
                    .into_iter()
                    .map(|item| item.oid)
                    .map(|oid| PinnedTree { oid })
                    .collect(),
            )),
            Self::Commits(items) => Self::Commits(canonicalize(items.clone())),
            Self::TreeRange { base, tip } => Self::TreeRange {
                base: base.clone(),
                tip: tip.clone(),
            },
            Self::CommitRange { base, tip } => Self::CommitRange {
                base: base.clone(),
                tip: tip.clone(),
            },
        }
    }

    /// Return the canonical identity: a single object uses its own OID;
    /// compound targets use facet-git-tree's frozen normal form.
    pub fn target_id(&self) -> Result<ObjectId, Error> {
        let empty = match self {
            Self::Blobs(items) => items.is_empty(),
            Self::Trees(items) => items.is_empty(),
            Self::Commits(items) => items.is_empty(),
            Self::TreeRange { .. } | Self::CommitRange { .. } => false,
        };
        if empty {
            return Err(Error::InvalidTarget("empty target collection".into()));
        }
        if let Some(oid) = self.single_oid() {
            return Ok(oid);
        }
        let descriptor = self.canonicalized().descriptor();
        normal_form::hash(&descriptor)
            .map(|(oid, _)| oid)
            .map_err(|error| Error::InvalidTarget(format!("failed to hash review target: {error}")))
    }

    fn single_oid(&self) -> Option<ObjectId> {
        match self {
            Self::Blobs(items) if items.len() == 1 => Some(items[0].oid.oid()),
            Self::Trees(items) if items.len() == 1 => Some(items[0].oid.oid()),
            Self::Commits(items) if items.len() == 1 => Some(items[0].oid()),
            _ => None,
        }
    }

    fn descriptor(&self) -> NormalForm {
        let (kind, values) = match self {
            Self::Blobs(items) => (
                "blobs",
                items
                    .iter()
                    .map(|item| NormalForm::Hash(item.oid.oid()))
                    .collect(),
            ),
            Self::Trees(items) => (
                "trees",
                items
                    .iter()
                    .map(|item| NormalForm::Hash(item.oid.oid()))
                    .collect(),
            ),
            Self::Commits(items) => (
                "commits",
                items
                    .iter()
                    .map(|item| NormalForm::Hash(item.oid()))
                    .collect(),
            ),
            Self::TreeRange { base, tip } => (
                "tree-range",
                vec![
                    NormalForm::Hash(base.oid.oid()),
                    NormalForm::Hash(tip.oid.oid()),
                ],
            ),
            Self::CommitRange { base, tip } => (
                "commit-range",
                vec![NormalForm::Hash(base.oid()), NormalForm::Hash(tip.oid())],
            ),
        };
        NormalForm::Struct(BTreeMap::from([
            ("kind".into(), NormalForm::Str(kind.into())),
            ("values".into(), NormalForm::List(values)),
        ]))
    }
}

fn split_two(rest: &str) -> Option<(&str, &str)> {
    let (a, b) = rest.split_once(':')?;
    (!a.is_empty() && !b.is_empty()).then_some((a, b))
}

fn parse_blobs(rest: &str) -> Result<ReviewTarget, Error> {
    if rest.is_empty() {
        return Err(Error::InvalidTarget("empty blobs collection".into()));
    }
    let items = rest
        .split(',')
        .map(|item| {
            let (path, oid) = item
                .rsplit_once(':')
                .ok_or_else(|| Error::InvalidTarget(item.into()))?;
            Ok(PinnedBlob {
                path: path.into(),
                oid: ExactOid::parse(oid)?,
            })
        })
        .collect::<Result<Vec<_>, Error>>()?;
    if items.is_empty() {
        return Err(Error::InvalidTarget("empty blobs collection".into()));
    }
    Ok(ReviewTarget::Blobs(canonicalize(items)))
}

fn parse_list<T, F>(rest: &str, make: F) -> Result<ReviewTarget, Error>
where
    F: FnOnce(Vec<ExactOid>) -> T,
    T: Into<ReviewTarget>,
{
    let values = if rest.is_empty() {
        vec![]
    } else {
        rest.split(',')
            .map(ExactOid::parse)
            .collect::<Result<Vec<_>, _>>()?
    };
    if values.is_empty() {
        return Err(Error::InvalidTarget("empty target collection".into()));
    }
    Ok(make(canonicalize(values)).into())
}

fn canonicalize<T: Ord>(mut values: Vec<T>) -> Vec<T> {
    values.sort();
    values
}

impl From<Vec<ExactOid>> for ReviewTarget {
    fn from(value: Vec<ExactOid>) -> Self {
        Self::Commits(value)
    }
}
impl From<Vec<PinnedTree>> for ReviewTarget {
    fn from(value: Vec<PinnedTree>) -> Self {
        Self::Trees(value)
    }
}

impl fmt::Display for ReviewTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Blobs(items) => write!(
                f,
                "blobs:{}",
                items
                    .iter()
                    .map(|i| format!("{}:{}", i.path, i.oid))
                    .collect::<Vec<_>>()
                    .join(",")
            ),
            Self::Trees(items) => write!(
                f,
                "trees:{}",
                items
                    .iter()
                    .map(|i| i.oid.to_string())
                    .collect::<Vec<_>>()
                    .join(",")
            ),
            Self::TreeRange { base, tip } => write!(f, "tree-range:{}:{}", base.oid, tip.oid),
            Self::Commits(items) => write!(
                f,
                "commits:{}",
                items
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(",")
            ),
            Self::CommitRange { base, tip } => write!(f, "commit-range:{}:{}", base, tip),
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

    fn oid(byte: u8) -> ExactOid {
        ExactOid::parse(&format!("{byte:02x}{byte:02x}{byte:02x}{byte:02x}{byte:02x}{byte:02x}{byte:02x}{byte:02x}{byte:02x}{byte:02x}{byte:02x}{byte:02x}{byte:02x}{byte:02x}{byte:02x}{byte:02x}{byte:02x}{byte:02x}{byte:02x}{byte:02x}")).unwrap()
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
                base: oid(0),
                tip: oid(1),
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
            target: ReviewTarget::Commits(vec![oid(0)]),
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
    fn every_shape_round_trips_and_has_identity() {
        let cases = [
            ReviewTarget::Blobs(vec![PinnedBlob {
                path: "src/lib.rs".into(),
                oid: oid(1),
            }]),
            ReviewTarget::Trees(vec![PinnedTree { oid: oid(2) }]),
            ReviewTarget::TreeRange {
                base: PinnedTree { oid: oid(2) },
                tip: PinnedTree { oid: oid(3) },
            },
            ReviewTarget::Commits(vec![oid(4)]),
            ReviewTarget::CommitRange {
                base: oid(4),
                tip: oid(5),
            },
        ];
        for target in cases {
            assert_eq!(ReviewTarget::parse(&target.to_string()).unwrap(), target);
            assert!(!target.target_id().unwrap().is_null());
        }
    }

    #[test]
    fn unordered_collections_are_canonical_and_path_is_not_identity() {
        let a = ReviewTarget::Blobs(vec![
            PinnedBlob {
                path: "a".into(),
                oid: oid(1),
            },
            PinnedBlob {
                path: "b".into(),
                oid: oid(2),
            },
        ]);
        let b = ReviewTarget::Blobs(vec![
            PinnedBlob {
                path: "renamed".into(),
                oid: oid(2),
            },
            PinnedBlob {
                path: "elsewhere".into(),
                oid: oid(1),
            },
        ]);
        assert_eq!(a.target_id().unwrap(), b.target_id().unwrap());
        assert_eq!(a.canonicalized(), a.canonicalized().canonicalized());
        assert_ne!(
            a.target_id().unwrap(),
            ReviewTarget::Blobs(vec![
                PinnedBlob {
                    path: "a".into(),
                    oid: oid(3)
                },
                PinnedBlob {
                    path: "b".into(),
                    oid: oid(2)
                }
            ])
            .target_id()
            .unwrap()
        );
    }

    #[test]
    fn malformed_oids_and_empty_collections_are_rejected() {
        for value in [
            "",
            "commit:deadbeef",
            "tree:",
            "blob:path:not-an-oid",
            "commits:",
        ] {
            assert!(ReviewTarget::parse(value).is_err(), "{value}");
        }
        assert!(ReviewTarget::Commits(vec![]).target_id().is_err());
        assert!(ReviewTarget::Trees(vec![]).target_id().is_err());
        assert!(ReviewTarget::Blobs(vec![]).target_id().is_err());
    }
}
