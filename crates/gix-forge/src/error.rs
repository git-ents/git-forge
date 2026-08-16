//! [`Error`]: everything `gix-forge`'s storage and query operations return.

/// Errors from `gix-forge`'s storage and query operations.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Failed at the `gix-store` layer (missing schema, git error, etc).
    #[error(transparent)]
    Store(#[from] gix_store::Error),
    /// An id is not a valid Git ref path.
    #[error(transparent)]
    InvalidId(#[from] gix_store::InvalidRefName),
    /// Failed at the `gix-query` layer.
    #[error(transparent)]
    Query(#[from] gix_query::QueryError),
    /// Failed storing or checking built-in query rules.
    #[error("failed to install built-in query rules: {0}")]
    QueryRules(String),
    /// Failed changing a repository reference during uninstall.
    #[error("failed to uninstall forge data: {0}")]
    Uninstall(String),
    /// Forge data is still present and must be removed first.
    #[error("cannot uninstall forge schemas while {0} data exists")]
    DataPresent(String),
    /// Failed capturing a `gix-anchor` binding.
    #[error(transparent)]
    Anchor(#[from] gix_anchor::Error),
    /// A `--target`/review-target string did not parse.
    #[error("invalid review target `{0}`")]
    InvalidTarget(String),
    /// The requested review does not exist.
    #[error("review `{0}` was not found")]
    ReviewNotFound(String),
    /// Reading annotated-tag references failed.
    #[error("could not read release tags: {0}")]
    ReleaseRepository(String),
    /// The repository has no usable authenticated signing configuration.
    #[error("missing signing configuration: {0}")]
    MissingSigner(String),
    /// A decision claim was unexpectedly written without a Git signature.
    #[error("review decision claim is unsigned")]
    UnsignedDecision,
    /// A release claim was unexpectedly written without a Git signature.
    #[error("release claim is unsigned")]
    UnsignedRelease,
    /// Failed to publish or read an attest claim.
    #[error(transparent)]
    Attest(#[from] gix_attest::Error),
    /// Authentication is required for the requested mutation.
    #[error("unauthorized: authentication is required to {capability}; provide --as <member-id>")]
    Unauthorized { capability: crate::Capability },
    /// The authenticated principal lacks the requested capability.
    #[error("forbidden: cannot {capability}; {reason}")]
    Forbidden {
        capability: crate::Capability,
        reason: &'static str,
    },
}
