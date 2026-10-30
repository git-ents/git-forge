//! Signed approvals and denials for review targets.

use std::io::Write as _;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use facet::Facet;
use gix::ObjectId;
use gix_attest::{Claims, Envelope, Target, register_claim_schema, register_key_schema};
use gix_store::{RefSegment, RepoStore, SignatureBytes, Signer, schema_of};

use crate::error::Error;
use crate::{Authorization, Capability, Ownership, Review, ReviewTarget};

/// The signed payload published for one review decision.
#[derive(Debug, Clone, PartialEq, Eq, Facet)]
pub struct ReviewDecision {
    /// The forge review this decision belongs to.
    pub review_id: String,
    /// Whether the reviewed material is approved or denied.
    pub verdict: ReviewVerdict,
    /// An optional explanation from the reviewer.
    pub message: Option<String>,
}

/// The two decisions a reviewer may publish.
#[derive(Debug, Clone, PartialEq, Eq, Facet)]
#[repr(u8)]
pub enum ReviewVerdict {
    /// Approve the reviewed material.
    Approve,
    /// Deny the reviewed material.
    Deny,
}

const KIND: &str = "review-decision";

/// A decision in a review's signed claim chain, including structural revocation
/// information supplied by `gix-attest`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewDecisionClaim {
    /// The signed claim's object id.
    pub claim_id: ObjectId,
    /// The decoded decision payload.
    pub decision: ReviewDecision,
    /// The claim that revokes this decision, if one exists.
    pub revoked_by: Option<ObjectId>,
    /// The key claim named by the decision's envelope.
    pub key_claim_id: ObjectId,
}

impl ReviewDecision {
    /// Read the resolved signed decisions for a review, newest first.
    pub fn claims_for_review(
        repo: &gix::Repository,
        review_id: &str,
    ) -> Result<Vec<ReviewDecisionClaim>, Error> {
        let review_store = crate::open_store(repo);
        let review = Review::load(&review_store, review_id)?
            .ok_or_else(|| Error::ReviewNotFound(review_id.to_owned()))?;
        let target = Target {
            kind: review.target.attest_kind().to_owned(),
            id: review.target.target_id()?.into(),
        };
        let store = RepoStore::open_with_layout(repo, gix_attest::layout());
        let claims = gix_attest::Claims::open(&store).resolve(&target)?;
        let kind = RefSegment::new(KIND).expect("built-in kind is valid");
        claims
            .into_iter()
            .filter(|claim| claim.envelope.payload_kind == KIND)
            .map(|claim| {
                let decision = store
                    .kind::<ReviewDecision>(kind.clone())
                    .decode(claim.envelope.payload.into())?;
                Ok(ReviewDecisionClaim {
                    claim_id: claim.id,
                    decision,
                    revoked_by: claim.revoked_by,
                    key_claim_id: claim.envelope.key.into(),
                })
            })
            .collect()
    }
}

impl ReviewDecision {
    /// Publish an authenticated approval for `review_id`.
    pub fn approve_as(
        repo: &gix::Repository,
        authorization: &Authorization,
        review_id: impl Into<String>,
        message: Option<String>,
    ) -> Result<ObjectId, Error> {
        Self::publish(
            repo,
            authorization,
            review_id.into(),
            ReviewVerdict::Approve,
            message,
        )
    }

    /// Publish an authenticated denial for `review_id`.
    pub fn deny_as(
        repo: &gix::Repository,
        authorization: &Authorization,
        review_id: impl Into<String>,
        message: Option<String>,
    ) -> Result<ObjectId, Error> {
        Self::publish(
            repo,
            authorization,
            review_id.into(),
            ReviewVerdict::Deny,
            message,
        )
    }

    fn publish(
        repo: &gix::Repository,
        authorization: &Authorization,
        review_id: String,
        verdict: ReviewVerdict,
        message: Option<String>,
    ) -> Result<ObjectId, Error> {
        authorization.check(Capability::ReviewCreate, Ownership::NotApplicable)?;
        let review_store = crate::open_store(repo);
        let review = Review::load(&review_store, &review_id)?
            .ok_or_else(|| Error::ReviewNotFound(review_id.clone()))?;
        let target = Target {
            kind: review.target.attest_kind().to_owned(),
            id: review.target.target_id()?.into(),
        };
        let decision = ReviewDecision {
            review_id,
            verdict,
            message,
        };

        let signing_key = SigningKey::from_config(repo)?;
        let store =
            RepoStore::open_with_layout(repo, gix_attest::layout()).with_signer(signing_key);
        register_claim_schema(&store)?;
        register_key_schema(&store)?;
        store
            .kind::<ReviewDecision>(RefSegment::new(KIND).expect("built-in kind is valid"))
            .schema()
            .put(&schema_of::<ReviewDecision>().map_err(gix_store::Error::from)?)?;
        let payload = store
            .kind::<ReviewDecision>(RefSegment::new(KIND).expect("built-in kind is valid"))
            .compile(&decision)?;
        let envelope = Envelope {
            target,
            payload: payload.into(),
            payload_kind: KIND.to_owned(),
            key: ObjectId::null(gix::hash::Kind::Sha1).into(),
        };
        let claim = Claims::open(&store).sign(&envelope)?;
        match store.signature(claim)? {
            Some(signature) if !signature.is_empty() => Ok(claim),
            _ => Err(Error::UnsignedDecision),
        }
    }
}

impl ReviewTarget {
    fn attest_kind(&self) -> &'static str {
        match self {
            Self::Blobs(items) if items.len() == 1 => "blob",
            Self::Blobs(_) => "blobs",
            Self::Trees(items) if items.len() == 1 => "tree",
            Self::Trees(_) => "trees",
            Self::TreeRange { .. } => "tree-range",
            Self::Commits(items) if items.len() == 1 => "commit",
            Self::Commits(_) => "commits",
            Self::CommitRange { .. } => "commit-range",
        }
    }
}

/// The configured SSH signing key used by Git's `gpg.format = ssh` backend.
pub(crate) struct SigningKey {
    private: PathBuf,
}

impl SigningKey {
    pub(crate) fn from_config(repo: &gix::Repository) -> Result<Self, Error> {
        let config = repo.config_snapshot();
        let format = config
            .string("gpg.format")
            .map(|value| value.to_string())
            .unwrap_or_else(|| "openpgp".to_owned());
        if format != "ssh" {
            return Err(Error::MissingSigner(
                "gpg.format must be ssh for forge decisions".to_owned(),
            ));
        }
        let value = config
            .string("user.signingkey")
            .ok_or_else(|| Error::MissingSigner("user.signingKey is not configured".to_owned()))?;
        let path = PathBuf::from(value.to_string());
        let private = if path.extension().is_some_and(|extension| extension == "pub") {
            path.with_extension("")
        } else {
            path
        };
        if !private.is_file() {
            return Err(Error::MissingSigner(format!(
                "private signing key does not exist: {}",
                private.display()
            )));
        }
        Ok(Self { private })
    }

    pub(crate) fn public_openssh(&self) -> Result<String, Error> {
        let public = self.private.with_extension("pub");
        std::fs::read_to_string(&public).map_err(|error| {
            Error::MissingSigner(format!(
                "cannot read public signing key {}: {error}",
                public.display()
            ))
        })
    }
}

impl Signer for SigningKey {
    type Error = std::io::Error;

    fn sign(&self, bytes: &[u8]) -> Result<SignatureBytes, Self::Error> {
        let mut child = Command::new("ssh-keygen")
            .args(["-Y", "sign", "-q", "-n", "git", "-f"])
            .arg(&self.private)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?;
        child
            .stdin
            .take()
            .ok_or_else(|| std::io::Error::other("ssh-keygen stdin was not piped"))?
            .write_all(bytes)?;
        let output = child.wait_with_output()?;
        if !output.status.success() {
            return Err(std::io::Error::other(format!(
                "ssh-keygen -Y sign failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }
        Ok(SignatureBytes::from(output.stdout))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::review::ExactOid;

    #[test]
    fn target_kinds_distinguish_scalar_and_compound_shapes() {
        let commit = ExactOid::parse("1111111111111111111111111111111111111111").unwrap();
        assert_eq!(
            ReviewTarget::Commits(vec![commit.clone()]).attest_kind(),
            "commit"
        );
        assert_eq!(
            ReviewTarget::Commits(vec![commit.clone(), commit]).attest_kind(),
            "commits"
        );
    }
}
