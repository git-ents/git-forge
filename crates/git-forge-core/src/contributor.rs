//! Contributor registry stored under `refs/forge/contributors`.
//!
//! Contributors are the identity foundation of the system. Every reference to
//! a person — issue author, assignee, comment attribution, approval signer —
//! uses the contributor ID string.
//!
//! ```text
//! refs/forge/contributors → commit → tree
//! ├── alice/
//! │   └── meta            # toml: name, email
//! ├── bob/
//! │   └── meta            # toml: name, email
//! ```

pub mod git2;

/// The ref under which the contributor registry is stored.
pub const CONTRIBUTORS_REF: &str = "refs/forge/contributors";

/// A registered contributor.
#[derive(Clone, Debug)]
pub struct Contributor {
    /// Stable short identifier (the directory name in the ref tree).
    pub id: String,
    /// Display name from `meta`.
    pub name: String,
    /// Email address from `meta`.
    pub email: String,
}

/// Operations on the contributor registry under [`CONTRIBUTORS_REF`].
pub trait Contributors {
    /// Return all registered contributors, sorted by ID.
    ///
    /// # Errors
    ///
    /// Returns `git2::Error` if the underlying repository operation fails.
    fn list_contributors(&self) -> Result<Vec<Contributor>, ::git2::Error>;

    /// Return the contributor with the given `id`, or `None` if not found.
    ///
    /// # Errors
    ///
    /// Returns `git2::Error` if the underlying repository operation fails.
    fn find_contributor(&self, id: &str) -> Result<Option<Contributor>, ::git2::Error>;

    /// Return the contributor whose email matches, or `None` if not found.
    ///
    /// # Errors
    ///
    /// Returns `git2::Error` if the underlying repository operation fails.
    fn find_contributor_by_email(&self, email: &str) -> Result<Option<Contributor>, ::git2::Error>;

    /// Add a new contributor with the given `id`, `name`, and `email`.
    ///
    /// Returns an error if a contributor with that ID already exists.
    ///
    /// # Errors
    ///
    /// Returns `git2::Error` if the contributor already exists or writing fails.
    fn add_contributor(&self, id: &str, name: &str, email: &str) -> Result<(), ::git2::Error>;
}
