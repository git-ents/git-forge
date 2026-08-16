//! Typed release documents keyed by annotated Git tags.

use std::fmt;
use std::str::FromStr;

use facet::Facet;
use gix::ObjectId;
use gix_attest::{AttestKey, Claims, Envelope, Target, register_claim_schema, register_key_schema};
use gix_store::{RefSegment, RepoStore, schema_of};

use crate::decision::SigningKey;
use crate::entity::{Entity, EntityOps};
use crate::{Authorization, Capability, Error, Ownership};

const CLAIM_KIND: &str = "release-claim";

/// An exact annotated-tag reference target.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Facet)]
pub struct ReleaseTarget {
    /// The short tag name, without `refs/tags/`.
    pub tag: String,
    /// The annotated tag object's full hexadecimal object id.
    pub oid: String,
}

impl ReleaseTarget {
    /// Parse `tag:<name>@<full-hex-tag-object-oid>`.
    pub fn parse(value: &str) -> Result<Self, Error> {
        let value = value
            .strip_prefix("tag:")
            .ok_or_else(|| Error::InvalidTarget(value.to_owned()))?;
        let (tag, oid) = value
            .rsplit_once('@')
            .ok_or_else(|| Error::InvalidTarget(value.to_owned()))?;
        if tag.is_empty() || ObjectId::from_hex(oid.as_bytes()).is_err() {
            return Err(Error::InvalidTarget(value.to_owned()));
        }
        Ok(Self {
            tag: tag.to_owned(),
            oid: oid.to_ascii_lowercase(),
        })
    }

    /// The object id of the annotated tag.
    pub fn object_id(&self) -> ObjectId {
        ObjectId::from_hex(self.oid.as_bytes()).expect("ReleaseTarget invariant")
    }
}

impl fmt::Display for ReleaseTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "tag:{}@{}", self.tag, self.oid)
    }
}

impl FromStr for ReleaseTarget {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

/// One optional attachment named by the release document.
#[derive(Debug, Clone, PartialEq, Eq, Facet)]
pub struct ReleaseAttachment {
    pub name: String,
    pub oid: String,
}

/// The typed document attached to one annotated tag.
#[derive(Debug, Clone, PartialEq, Eq, Facet)]
pub struct Release {
    pub id: String,
    pub target: ReleaseTarget,
    pub body: String,
    pub attachments: Vec<ReleaseAttachment>,
    pub edit: Option<String>,
}

#[derive(Debug, Clone, Facet)]
pub struct StoredRelease {
    target: ReleaseTarget,
    body: String,
    attachments: Vec<ReleaseAttachment>,
    edit: Option<String>,
}

impl Entity for Release {
    const KIND: &'static str = "release";
    type Stored = StoredRelease;

    fn id(&self) -> &str {
        &self.id
    }
    fn to_stored(&self) -> StoredRelease {
        StoredRelease {
            target: self.target.clone(),
            body: self.body.clone(),
            attachments: self.attachments.clone(),
            edit: self.edit.clone(),
        }
    }
    fn from_stored(id: String, stored: StoredRelease) -> Self {
        Self {
            id,
            target: stored.target,
            body: stored.body,
            attachments: stored.attachments,
            edit: stored.edit,
        }
    }
    fn ownership(&self, authorization: &Authorization) -> Ownership {
        match authorization.principal() {
            crate::Principal::Member(member) if self.id == member.as_str() => Ownership::Owned,
            _ => Ownership::NotOwned,
        }
    }
}

impl Release {
    /// Publish the release schema after checking the release-create capability.
    pub fn ensure_schema_as(
        store: &RepoStore<'_>,
        authorization: &Authorization,
    ) -> Result<ObjectId, Error> {
        authorization.check(Capability::ReleaseCreate, Ownership::NotApplicable)?;
        Self::kind(store)
            .schema()
            .put(&schema_of::<StoredRelease>().map_err(gix_store::Error::from)?)
            .map_err(Error::from)
    }

    /// Load the release document keyed by tag name.
    pub fn load_from_repo(repo: &gix::Repository, tag: &str) -> Result<Option<Self>, Error> {
        <Self as EntityOps>::load_from_repo(repo, tag)
    }

    /// Save a release body and attachments after authorization.
    pub fn save_in_repo_as(
        &self,
        repo: &gix::Repository,
        authorization: &Authorization,
    ) -> Result<ObjectId, Error> {
        authorization.check(Capability::ReleaseUpdate, Ownership::Owned)?;
        let store = crate::open_store(repo);
        Self::ensure_schema_as(&store, authorization)?;
        <Self as EntityOps>::save_as(self, &store, authorization)
    }

    /// All annotated tags, ordered by tag name, with their immutable tag object ids.
    pub fn annotated_tags(repo: &gix::Repository) -> Result<Vec<ReleaseTarget>, Error> {
        let mut tags = repo
            .references()
            .map_err(|error| Error::ReleaseRepository(error.to_string()))?
            .tags()
            .map_err(|error| Error::ReleaseRepository(error.to_string()))?
            .flatten()
            .filter_map(|mut reference| {
                let tag = reference.peel_to_tag().ok()?;
                Some(ReleaseTarget {
                    tag: reference.name().shorten().to_string(),
                    oid: tag.id.to_string(),
                })
            })
            .collect::<Vec<_>>();
        tags.sort_by(|a, b| a.tag.cmp(&b.tag));
        Ok(tags)
    }

    /// Resolve the signed release ledger for the tag, including crypto validity.
    pub fn claims_for_tag(
        repo: &gix::Repository,
        target: &ReleaseTarget,
    ) -> Result<Vec<ReleaseClaim>, Error> {
        let store = RepoStore::open_with_layout(repo, gix_attest::layout());
        let claims = Claims::open(&store).resolve(&Target {
            kind: "tag".into(),
            id: target.object_id().into(),
        })?;
        let kind = RefSegment::new(CLAIM_KIND).expect("built-in claim kind is valid");
        claims
            .into_iter()
            .filter(|claim| claim.envelope.payload_kind == CLAIM_KIND)
            .map(|claim| {
                let payload = store
                    .kind::<StoredReleaseClaim>(kind.clone())
                    .decode(claim.envelope.payload.into())?;
                let verified =
                    Claims::open(&store).verify(&claim)? == gix_attest::Verdict::Verified;
                let date = repo
                    .find_commit(claim.id)
                    .ok()
                    .and_then(|commit| commit.time().ok())
                    .map_or_else(
                        || "unknown date".into(),
                        |time| time.format_or_unix(gix::date::time::format::SHORT),
                    );
                Ok(ReleaseClaim {
                    claim_id: claim.id,
                    signer: payload.signer,
                    key: claim.envelope.key.into(),
                    date,
                    valid: verified,
                    revoked: claim.revoked_by,
                })
            })
            .collect()
    }

    /// Publish one signed claim over the annotated tag.
    pub fn publish_claim_as(
        repo: &gix::Repository,
        authorization: &Authorization,
        target: &ReleaseTarget,
    ) -> Result<ObjectId, Error> {
        authorization.check(Capability::ReleasePublish, Ownership::NotApplicable)?;
        let signer = match authorization.principal() {
            crate::Principal::Member(id) => id.as_str().to_owned(),
            _ => {
                return Err(Error::Unauthorized {
                    capability: Capability::ReleasePublish,
                });
            }
        };
        let signing_key = SigningKey::from_config(repo)?;
        let public_key = signing_key.public_openssh()?;
        let store =
            RepoStore::open_with_layout(repo, gix_attest::layout()).with_signer(signing_key);
        register_claim_schema(&store)?;
        register_key_schema(&store)?;
        let kind = RefSegment::new(CLAIM_KIND).expect("built-in claim kind is valid");
        store
            .kind::<StoredReleaseClaim>(kind.clone())
            .schema()
            .put(&schema_of::<StoredReleaseClaim>().map_err(gix_store::Error::from)?)?;
        let key = AttestKey::from_openssh(&public_key, false)?;
        let key_claim = Claims::open(&store).add_key(&key)?;
        let payload = store
            .kind::<StoredReleaseClaim>(kind)
            .compile(&StoredReleaseClaim { signer })?;
        let envelope = Envelope {
            target: Target {
                kind: "tag".into(),
                id: target.object_id().into(),
            },
            payload: payload.into(),
            payload_kind: CLAIM_KIND.into(),
            key: key_claim.into(),
        };
        let claim = Claims::open(&store).sign(&envelope)?;
        match store.signature(claim)? {
            Some(signature) if !signature.is_empty() => Ok(claim),
            _ => Err(Error::UnsignedRelease),
        }
    }
}

#[derive(Debug, Clone, Facet)]
struct StoredReleaseClaim {
    signer: String,
}

/// One resolved signed release claim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseClaim {
    pub claim_id: ObjectId,
    pub signer: String,
    pub key: ObjectId,
    pub date: String,
    pub valid: bool,
    pub revoked: Option<ObjectId>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_target_round_trips() {
        let target =
            ReleaseTarget::parse("tag:v1.2.3@1111111111111111111111111111111111111111").unwrap();
        assert_eq!(
            target.to_string(),
            "tag:v1.2.3@1111111111111111111111111111111111111111"
        );
        assert_eq!(ReleaseTarget::parse(&target.to_string()).unwrap(), target);
    }
}
