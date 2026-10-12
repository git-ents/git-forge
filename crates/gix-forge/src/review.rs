//! [`Review`]: a forge doc requesting review of a target, plus
//! [`ReviewTarget`], the typed vocabulary a review request names.

use std::collections::BTreeMap;
use std::fmt;

use facet::Facet;
use facet_git_tree::normal_form::{self, NormalForm};
use facet_git_tree::{RawBlob, RawTree};
use gix::ObjectId;
use gix_store::{RefSegment, RepoStore, Typed};

use crate::comment::Commentable;
use crate::entity::{Entity, EntityOps};
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
    target: StoredReviewTarget,
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
            target: self.target.canonicalized().into(),
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
            target: stored.target.into(),
            edit: stored.edit,
        }
    }
}

impl Commentable for Review {
    fn comment_subject(&self) -> (&'static str, &str) {
        (Review::KIND, &self.id)
    }
}

impl Review {
    fn kind<'a>(
        store: &'a RepoStore<'a>,
    ) -> gix_store::Kind<'a, Typed<StoredReview>, gix_store::GixRefStore<'a>, &'a gix::OdbHandle>
    {
        store.kind(RefSegment::new(Self::KIND).expect("built-in ref segment is valid"))
    }

    /// Publish the current review schema without asking facet-git-tree to
    /// describe `RawBlob`.
    pub fn ensure_schema(store: &RepoStore<'_>) -> Result<ObjectId, Error> {
        Self::kind(store)
            .schema()
            .put(&facet_git_tree::schema_of::<ReviewSchema>().map_err(gix_store::Error::from)?)
            .map_err(Error::from)
    }

    /// Authorized schema publication for review callers.
    pub fn ensure_schema_as(
        store: &RepoStore<'_>,
        authorization: &Authorization,
    ) -> Result<ObjectId, Error> {
        authorization.check(crate::Capability::ReviewCreate, Ownership::NotApplicable)?;
        Self::ensure_schema(store)
    }

    /// Read a review, falling back only to the exact pre-retention shape.
    /// No object is rewritten and the target OIDs are copied verbatim.
    pub fn load(store: &RepoStore<'_>, id: &str) -> Result<Option<Self>, Error> {
        let name = gix_store::RefPath::new(id)?;
        match Self::kind(store).read(&name) {
            Ok(state) => Ok(state
                .value()
                .map(|stored| Self::from_stored(id.to_owned(), stored))),
            Err(current) => {
                let legacy = store.kind::<LegacyStoredReview>(
                    RefSegment::new(Self::KIND).expect("built-in ref segment is valid"),
                );
                match legacy.read(&name) {
                    Ok(state) => Ok(state.value().map(|stored| {
                        Self::from_stored(id.to_owned(), StoredReview::from(stored))
                    })),
                    Err(_) => Err(current.into()),
                }
            }
        }
    }

    /// Repository-backed counterpart to [`load`](Self::load).
    pub fn load_from_repo(repo: &gix::Repository, id: &str) -> Result<Option<Self>, Error> {
        Self::load(&crate::open_store(repo), id)
    }

    /// Store a review using the current canonical retained representation.
    pub fn save_as(
        &self,
        store: &RepoStore<'_>,
        authorization: &Authorization,
    ) -> Result<ObjectId, Error> {
        authorization.check(
            crate::Capability::ReviewUpdate,
            self.ownership(authorization),
        )?;
        // Keep the shared content-identity alias handling used by the other
        // entities. In particular, a migrated caller name may coexist with a
        // canonical ref; updating through either name must retarget every
        // name with CAS protection instead of leaving a stale publication.
        <Self as EntityOps>::save_as(self, store, authorization)
    }

    pub fn create_in_repo_as(
        &self,
        repo: &gix::Repository,
        authorization: &Authorization,
    ) -> Result<String, Error> {
        authorization.check(crate::Capability::ReviewCreate, Ownership::NotApplicable)?;
        let store = crate::open_store(repo);
        Self::ensure_schema(&store)?;
        let mut entity = self.clone();
        if let crate::Principal::Member(member) = authorization.principal() {
            entity.attribute_to(member.as_str());
        }
        let commit = Self::kind(&store).write(&entity.to_stored()).anonymous()?;
        Ok(gix_store::entity_name(commit).to_string())
    }

    pub fn save_in_repo_as(
        &self,
        repo: &gix::Repository,
        authorization: &Authorization,
    ) -> Result<ObjectId, Error> {
        authorization.check(
            crate::Capability::ReviewUpdate,
            self.ownership(authorization),
        )?;
        let store = crate::open_store(repo);
        Self::ensure_schema(&store)?;
        self.save_as(&store, authorization)
    }

    pub fn delete_as(
        repo: &gix::Repository,
        id: &str,
        authorization: &Authorization,
    ) -> Result<bool, Error> {
        let store = crate::open_store(repo);
        let entity = Self::load(&store, id)?;
        let ownership = entity
            .as_ref()
            .map_or(Ownership::Owned, |review| review.ownership(authorization));
        authorization.check(crate::Capability::ReviewDelete, ownership)?;
        // Deletion is likewise by logical document identity, not merely by
        // the spelling supplied by the caller. This removes canonical and
        // migrated aliases together while retaining gix-store's ref/CAS
        // semantics.
        <Self as EntityOps>::delete_as(repo, id, authorization)
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

#[derive(Debug, Clone, Facet)]
#[repr(u8)]
enum StoredReviewTarget {
    Blobs(Vec<StoredBlob>),
    Trees(Vec<StoredTree>),
    TreeRange { base: RawTree, tip: RawTree },
    Commits(Vec<ExactOid>),
    CommitRange { base: ExactOid, tip: ExactOid },
}

#[derive(Debug, Clone, Facet)]
struct StoredBlob {
    path: String,
    oid: RawBlob,
}

#[derive(Debug, Clone, Facet)]
struct StoredTree {
    oid: RawTree,
}

/// The schema-only shape for the current retained target encoding.
///
/// `RawBlob` is a serializer-level passthrough, but facet-git-tree does not
/// expose a corresponding schema node. Bytes have the same schema-level
/// representation and still validate the actual `RawBlob` entry on writes and
/// reads. The runtime value remains `RawBlob`, so blob reachability and object
/// identity are unchanged.
#[derive(Debug, Facet)]
struct ReviewSchema {
    status: String,
    body: String,
    reviewers: Vec<String>,
    requesters: Vec<String>,
    target: ReviewSchemaTarget,
    edit: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Facet)]
#[repr(u8)]
enum ReviewSchemaTarget {
    Blobs(Vec<ReviewSchemaBlob>),
    Trees(Vec<ReviewSchemaTree>),
    TreeRange { base: RawTree, tip: RawTree },
    Commits(Vec<ExactOid>),
    CommitRange { base: ExactOid, tip: ExactOid },
}

#[derive(Debug, Facet)]
struct ReviewSchemaBlob {
    path: String,
    oid: Vec<u8>,
}

#[derive(Debug, Facet)]
struct ReviewSchemaTree {
    oid: RawTree,
}

/// The target shape written before retention used exact OID strings.
#[derive(Debug, Facet)]
struct LegacyStoredReview {
    status: String,
    body: String,
    reviewers: Vec<String>,
    requesters: Vec<String>,
    target: ReviewTarget,
    edit: Option<String>,
}

impl From<LegacyStoredReview> for StoredReview {
    fn from(legacy: LegacyStoredReview) -> Self {
        StoredReview {
            status: legacy.status,
            body: legacy.body,
            reviewers: legacy.reviewers,
            requesters: legacy.requesters,
            target: legacy.target.into(),
            edit: legacy.edit,
        }
    }
}

impl From<ReviewTarget> for StoredReviewTarget {
    fn from(target: ReviewTarget) -> Self {
        match target {
            ReviewTarget::Blobs(items) => Self::Blobs(
                items
                    .into_iter()
                    .map(|item| StoredBlob {
                        path: item.path,
                        oid: RawBlob::new(item.oid.oid()),
                    })
                    .collect(),
            ),
            ReviewTarget::Trees(items) => Self::Trees(
                items
                    .into_iter()
                    .map(|item| StoredTree {
                        oid: RawTree::new(item.oid.oid()),
                    })
                    .collect(),
            ),
            ReviewTarget::TreeRange { base, tip } => Self::TreeRange {
                base: RawTree::new(base.oid.oid()),
                tip: RawTree::new(tip.oid.oid()),
            },
            ReviewTarget::Commits(items) => Self::Commits(items),
            ReviewTarget::CommitRange { base, tip } => Self::CommitRange { base, tip },
        }
    }
}

impl From<StoredReviewTarget> for ReviewTarget {
    fn from(target: StoredReviewTarget) -> Self {
        match target {
            StoredReviewTarget::Blobs(items) => Self::Blobs(
                items
                    .into_iter()
                    .map(|item| PinnedBlob {
                        path: item.path,
                        oid: ExactOid::parse(&item.oid.oid().to_string())
                            .expect("RawBlob contains an exact object id"),
                    })
                    .collect(),
            ),
            StoredReviewTarget::Trees(items) => Self::Trees(
                items
                    .into_iter()
                    .map(|item| PinnedTree {
                        oid: ExactOid::parse(&item.oid.oid().to_string())
                            .expect("RawTree contains an exact object id"),
                    })
                    .collect(),
            ),
            StoredReviewTarget::TreeRange { base, tip } => Self::TreeRange {
                base: PinnedTree {
                    oid: ExactOid::parse(&base.oid().to_string())
                        .expect("RawTree contains an exact object id"),
                },
                tip: PinnedTree {
                    oid: ExactOid::parse(&tip.oid().to_string())
                        .expect("RawTree contains an exact object id"),
                },
            },
            StoredReviewTarget::Commits(items) => Self::Commits(items),
            StoredReviewTarget::CommitRange { base, tip } => Self::CommitRange { base, tip },
        }
    }
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
        let target = self.canonicalized();
        let empty = match &target {
            Self::Blobs(items) => items.is_empty(),
            Self::Trees(items) => items.is_empty(),
            Self::Commits(items) => items.is_empty(),
            Self::TreeRange { .. } | Self::CommitRange { .. } => false,
        };
        if empty {
            return Err(Error::InvalidTarget("empty target collection".into()));
        }
        if let Some(oid) = target.single_oid() {
            return Ok(oid);
        }
        let descriptor = target.descriptor();
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

    use crate::{
        Authorization, Comment, Commentable, EntityOps, Principal, ReviewDecision, open_store,
        query_goal,
    };
    use facet_git_tree::{EntryKind, ObjectStore, serialize_into};
    use gix_store::{RefEdit, RefStore};
    use std::io::Write;
    use std::process::Command;

    fn auth() -> Authorization {
        Authorization::new(Principal::member_id("alice"))
    }

    fn oid(byte: u8) -> ExactOid {
        ExactOid::parse(&format!("{byte:02x}{byte:02x}{byte:02x}{byte:02x}{byte:02x}{byte:02x}{byte:02x}{byte:02x}{byte:02x}{byte:02x}{byte:02x}{byte:02x}{byte:02x}{byte:02x}{byte:02x}{byte:02x}{byte:02x}{byte:02x}{byte:02x}{byte:02x}")).unwrap()
    }

    #[test]
    fn old_retention_free_reviews_remain_readable_without_rewriting() {
        let dir = tempfile::tempdir().expect("tempdir");
        test_support::init_repo(dir.path());
        let repo = gix::open(dir.path()).expect("open repo");
        let store = open_store(&repo);
        let legacy = store.kind::<LegacyStoredReview>(
            RefSegment::new(Review::KIND).expect("built-in ref segment is valid"),
        );
        legacy
            .schema()
            .put(&facet_git_tree::schema_of::<LegacyStoredReview>().expect("legacy schema"))
            .expect("publish legacy schema");
        let target = ReviewTarget::Blobs(vec![PinnedBlob {
            path: "old.txt".into(),
            oid: oid(7),
        }]);
        legacy
            .put(
                &gix_store::RefPath::new("legacy-1").expect("review id"),
                &LegacyStoredReview {
                    status: "open".into(),
                    body: "legacy".into(),
                    reviewers: vec!["bob".into()],
                    requesters: vec!["alice".into()],
                    target: target.clone(),
                    edit: None,
                },
            )
            .expect("write legacy review");

        let loaded = Review::load(&store, "legacy-1")
            .expect("load legacy review")
            .expect("legacy review exists");
        assert_eq!(loaded.target, target);
        assert_eq!(loaded.id, "legacy-1");
    }

    /// Build a migrated review with a canonical document-tree ref and a
    /// caller/legacy alias pointing at the same publication.
    fn migrated_alias_fixture() -> (tempfile::TempDir, gix::Repository, String, String) {
        let dir = tempfile::tempdir().expect("tempdir");
        test_support::init_repo(dir.path());
        let repo = gix::open(dir.path()).expect("open repo");
        let store = open_store(&repo);
        let segment = RefSegment::new(Review::KIND).expect("review segment");
        let legacy_kind = store.kind::<LegacyStoredReview>(segment);
        legacy_kind
            .schema()
            .put(&facet_git_tree::schema_of::<LegacyStoredReview>().expect("legacy schema"))
            .expect("publish legacy schema");

        let legacy = LegacyStoredReview {
            status: "open".to_owned(),
            body: "Migrated review contains the sentinel".to_owned(),
            reviewers: vec!["carol".to_owned()],
            requesters: vec!["alice".to_owned()],
            target: ReviewTarget::Commits(vec![oid(7)]),
            edit: None,
        };
        let commit = legacy_kind
            .write(&legacy)
            .anonymous()
            .expect("write legacy review");

        Review::ensure_schema_as(&store, &auth()).expect("publish current schema");
        let current_kind = Review::kind(&store);
        let commit_name = gix_store::entity_name(commit);
        let canonical = gix_store::RefPath::new(
            repo.find_commit(commit)
                .expect("legacy commit")
                .tree_id()
                .expect("legacy root tree")
                .detach()
                .to_string(),
        )
        .expect("canonical tree id");
        let canonical_ref = current_kind.reference(&canonical);
        if let Some(expected) = store
            .refs()
            .read(&canonical_ref)
            .expect("read canonical ref")
        {
            store
                .refs()
                .apply(RefEdit::Delete {
                    name: canonical_ref.clone(),
                    expected,
                })
                .expect("remove auto-published canonical ref");
        }
        store
            .refs()
            .apply(RefEdit::Create {
                name: canonical_ref,
                new: commit,
            })
            .expect("publish canonical ref");
        store
            .refs()
            .apply(RefEdit::Delete {
                name: current_kind.reference(&commit_name),
                expected: commit,
            })
            .expect("remove pre-migration ref name");
        let alias = gix_store::RefPath::new("legacy-review-id").expect("alias id");
        store
            .refs()
            .apply(RefEdit::Create {
                name: current_kind.reference(&alias),
                new: commit,
            })
            .expect("publish caller alias");

        (dir, repo, canonical.to_string(), alias.to_string())
    }

    #[test]
    fn migrated_aliases_are_one_review_across_crud_and_broad_queries() {
        let (_dir, repo, canonical, alias) = migrated_alias_fixture();
        let store = open_store(&repo);

        let loaded = Review::load(&store, &alias)
            .expect("load alias")
            .expect("alias exists");
        assert_eq!(loaded.id, alias, "direct loads preserve the caller id");
        assert_eq!(loaded.body, "Migrated review contains the sentinel");

        let source_names = Review::source_names(&repo).expect("source names");
        assert_eq!(source_names.len(), 2);
        assert!(source_names.contains(&canonical));
        assert!(source_names.contains(&alias));
        assert_eq!(Review::list(&repo).expect("list"), vec![alias.clone()]);
        let all = Review::load_all(&store).expect("load all");
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].id, alias);
        assert_eq!(
            query_goal(&repo, "review(Id)", &["Id"])
                .expect("query")
                .len(),
            1
        );
        assert_eq!(
            crate::search_review(&repo, None, None, Some("SENTINEL")).expect("search"),
            vec![alias.clone()]
        );

        let extra_alias = gix_store::RefPath::new("second-legacy-review-id").expect("second alias");
        let commit = Review::kind(&store)
            .read(gix_store::RefPath::new(&canonical).expect("canonical path"))
            .expect("read canonical entry")
            .present()
            .expect("canonical entry")
            .commit;
        store
            .refs()
            .apply(RefEdit::Create {
                name: Review::kind(&store).reference(&extra_alias),
                new: commit,
            })
            .expect("publish second alias");

        let mut changed = Review::load(&store, &alias)
            .expect("load alias")
            .expect("alias exists");
        changed.status = "closed".to_owned();
        changed.body = "Updated through the legacy alias".to_owned();
        changed
            .save_as(&store, &auth())
            .expect("save through alias");
        assert_eq!(
            Review::load(&store, &canonical)
                .expect("load canonical")
                .expect("canonical exists")
                .status,
            "closed"
        );
        assert_eq!(
            Review::load(&store, &extra_alias.to_string())
                .expect("load extra alias")
                .expect("extra alias exists")
                .body,
            "Updated through the legacy alias"
        );
        assert_eq!(
            Review::history(&repo, &canonical).expect("canonical history"),
            Review::history(&repo, &alias).expect("alias history")
        );

        assert!(Review::delete_as(&repo, &alias, &auth()).expect("delete alias"));
        assert!(Review::list(&repo).expect("list after delete").is_empty());
        for id in [&canonical, &alias, &extra_alias.to_string()] {
            assert!(
                Review::load(&store, id)
                    .expect("load after delete")
                    .is_none(),
                "{id} should be deleted"
            );
        }
    }

    #[test]
    fn alias_only_legacy_review_refs_are_discoverable() {
        let (_dir, repo, canonical, alias) = migrated_alias_fixture();
        let store = open_store(&repo);
        let canonical_path = gix_store::RefPath::new(&canonical).expect("canonical path");
        let commit = Review::kind(&store)
            .read(&canonical_path)
            .expect("read canonical ref")
            .present()
            .expect("canonical ref exists")
            .commit;
        store
            .refs()
            .apply(RefEdit::Delete {
                name: Review::kind(&store).reference(&canonical_path),
                expected: commit,
            })
            .expect("remove canonical ref");

        assert_eq!(Review::list(&repo).expect("list"), vec![alias.clone()]);
        let all = Review::load_all(&store).expect("load all");
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].id, alias);
        assert_eq!(
            query_goal(&repo, "review(Id)", &["Id"])
                .expect("query")
                .len(),
            1
        );
        assert_eq!(
            crate::search_review(&repo, None, None, Some("sentinel")).expect("search"),
            vec![alias]
        );
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

    fn stored_review(target: ReviewTarget) -> StoredReview {
        StoredReview {
            status: "open".into(),
            body: "body".into(),
            reviewers: vec![],
            requesters: vec![],
            target: target.canonicalized().into(),
            edit: None,
        }
    }

    fn child_tree(store: &ObjectStore, root: &ObjectId, name: &str) -> ObjectId {
        store
            .get_tree(root)
            .expect("root tree")
            .expect("root tree is present")
            .into_iter()
            .find(|entry| entry.filename == name)
            .expect("tree entry")
            .oid
    }

    #[test]
    fn retained_blob_and_tree_targets_are_direct_edges() {
        let blob = oid(1).oid();
        let tree = oid(2).oid();

        let blob_store = ObjectStore::default();
        let blob_root = serialize_into(
            &stored_review(ReviewTarget::Blobs(vec![PinnedBlob {
                path: "src/lib.rs".into(),
                oid: ExactOid::parse(&blob.to_string()).unwrap(),
            }])),
            &blob_store,
        )
        .expect("serialize blob review");
        let blob_target = child_tree(&blob_store, &blob_root, "target");
        let blob_variant = child_tree(&blob_store, &blob_target, "Blobs");
        let blob_item = child_tree(&blob_store, &blob_variant, "0000");
        let blob_entry = blob_store
            .get_tree(&blob_item)
            .expect("blob item tree")
            .expect("blob item tree is present")
            .into_iter()
            .find(|entry| entry.filename == "oid")
            .expect("blob oid entry");
        assert_eq!(blob_entry.mode.kind(), EntryKind::Blob);
        assert_eq!(blob_entry.oid, blob);

        let tree_store = ObjectStore::default();
        let tree_root = serialize_into(
            &stored_review(ReviewTarget::Trees(vec![PinnedTree {
                oid: ExactOid::parse(&tree.to_string()).unwrap(),
            }])),
            &tree_store,
        )
        .expect("serialize tree review");
        let tree_target = child_tree(&tree_store, &tree_root, "target");
        let tree_variant = child_tree(&tree_store, &tree_target, "Trees");
        let tree_item = child_tree(&tree_store, &tree_variant, "0000");
        let tree_entry = tree_store
            .get_tree(&tree_item)
            .expect("tree item tree")
            .expect("tree item tree is present")
            .into_iter()
            .find(|entry| entry.filename == "oid")
            .expect("tree oid entry");
        assert_eq!(tree_entry.mode.kind(), EntryKind::Tree);
        assert_eq!(tree_entry.oid, tree);
    }

    #[test]
    fn tree_range_retains_base_and_tip_as_direct_entries() {
        let base = oid(2).oid();
        let tip = oid(3).oid();
        let store = ObjectStore::default();
        let root = serialize_into(
            &stored_review(ReviewTarget::TreeRange {
                base: PinnedTree {
                    oid: ExactOid::parse(&base.to_string()).unwrap(),
                },
                tip: PinnedTree {
                    oid: ExactOid::parse(&tip.to_string()).unwrap(),
                },
            }),
            &store,
        )
        .expect("serialize tree range review");
        let target = child_tree(&store, &root, "target");
        let range = child_tree(&store, &target, "TreeRange");
        let entries = store
            .get_tree(&range)
            .expect("tree range tree")
            .expect("tree range tree is present");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].filename, "base");
        assert_eq!(entries[0].mode.kind(), EntryKind::Tree);
        assert_eq!(entries[0].oid, base);
        assert_eq!(entries[1].filename, "tip");
        assert_eq!(entries[1].mode.kind(), EntryKind::Tree);
        assert_eq!(entries[1].oid, tip);
    }

    #[test]
    fn commit_targets_preserve_ids_without_reachability_edges() {
        let commit = oid(4).oid();
        let store = ObjectStore::default();
        let root = serialize_into(
            &stored_review(ReviewTarget::Commits(vec![
                ExactOid::parse(&commit.to_string()).unwrap(),
            ])),
            &store,
        )
        .expect("serialize commit review");
        let target = child_tree(&store, &root, "target");
        let commits = child_tree(&store, &target, "Commits");
        let entry = &store
            .get_tree(&commits)
            .expect("commit collection tree")
            .expect("commit collection tree is present")[0];
        assert_eq!(entry.mode.kind(), EntryKind::Tree);
        assert_ne!(entry.oid, commit);

        let stored: StoredReview = facet_git_tree::deserialize(&root, &store).expect("deserialize");
        assert_eq!(
            ReviewTarget::from(stored.target),
            ReviewTarget::Commits(vec![ExactOid::parse(&commit.to_string()).unwrap()])
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

    #[test]
    fn review_mutations_do_not_touch_comments_or_attest_decisions() {
        let dir = tempfile::tempdir().expect("tempdir");
        test_support::init_repo(dir.path());
        let key = dir.path().join("review-signing-key");
        let status = Command::new("ssh-keygen")
            .args(["-q", "-t", "ed25519", "-N", ""])
            .arg("-f")
            .arg(&key)
            .status()
            .expect("run ssh-keygen");
        assert!(status.success(), "ssh-keygen failed");
        std::fs::OpenOptions::new()
            .append(true)
            .open(dir.path().join(".git/config"))
            .expect("open repository config")
            .write_all(
                format!(
                    "\n[gpg]\n\tformat = ssh\n[user]\n\tsigningkey = {}\n",
                    key.display()
                )
                .as_bytes(),
            )
            .expect("configure signing key");
        let repo = gix::open(dir.path()).expect("open repo");
        let authorization = auth();
        let mut review = Review {
            id: String::new(),
            status: "open".into(),
            body: "original body".into(),
            reviewers: vec![],
            requesters: vec![],
            target: ReviewTarget::Commits(vec![oid(1)]),
            edit: None,
        };
        let review_id = review
            .create_in_repo_as(&repo, &authorization)
            .expect("create review");
        review.id = review_id;
        review = Review::load_from_repo(&repo, &review.id)
            .expect("load created review")
            .expect("created review exists");

        let comment_id = review
            .add_comment_as(&repo, &authorization, "review-level comment")
            .expect("create review comment");
        let comment_before = Comment::load_from_repo(&repo, &comment_id)
            .expect("load comment")
            .expect("comment exists");
        assert_eq!(
            comment_before.subject.as_deref(),
            Some(format!("review:{}", review.id).as_str())
        );
        assert!(comment_before.binding.is_none());

        let claim = ReviewDecision::approve_as(
            &repo,
            &authorization,
            &review.id,
            Some("approved independently".into()),
        )
        .expect("publish signed decision");
        assert!(repo.find_commit(claim).is_ok(), "claim is not a commit");

        let mut edited = review.clone();
        edited.body = "edited body".into();
        edited
            .save_in_repo_as(&repo, &authorization)
            .expect("edit review");
        let comment_after_edit = Comment::load_from_repo(&repo, &comment_id)
            .expect("reload comment after edit")
            .expect("comment survives review edit");
        assert_eq!(comment_after_edit.subject, comment_before.subject);
        assert_eq!(comment_after_edit.body, comment_before.body);
        assert!(
            repo.find_commit(claim).is_ok(),
            "decision survives review edit"
        );

        assert!(Review::delete_as(&repo, &review.id, &authorization).expect("delete review"));
        assert!(
            Review::load_from_repo(&repo, &review.id)
                .expect("load deleted review")
                .is_none()
        );
        let comment_after_delete = Comment::load_from_repo(&repo, &comment_id)
            .expect("reload comment after delete")
            .expect("comment survives review deletion");
        assert_eq!(
            comment_after_delete.subject.as_deref(),
            Some(format!("review:{}", review.id).as_str())
        );
        assert!(
            repo.find_commit(claim).is_ok(),
            "decision survives review deletion"
        );
    }
}
