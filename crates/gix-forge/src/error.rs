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
    /// Failed capturing a `gix-anchor` binding.
    #[error(transparent)]
    Anchor(#[from] gix_anchor::Error),
    /// A `--target`/review-target string did not parse.
    #[error("invalid review target `{0}`")]
    InvalidTarget(String),
}
