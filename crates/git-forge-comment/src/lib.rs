//! Code, review, and issue comments anchored to Git objects.
//!
//! A comment is stored as a Git commit object. Trailers on the commit message
//! carry structured metadata (anchor, resolved state, parent comment OID, etc.).
//!
//! Refs live under `refs/forge/comments/`.

pub mod cli;
pub mod exe;
pub mod git2;

/// Ref prefix under which comment refs are stored.
pub const COMMENTS_REF_PREFIX: &str = "refs/forge/comments/";

/// Returns the ref name for comments on a specific issue.
#[must_use]
pub fn issue_comments_ref(id: u64) -> String {
    format!("{COMMENTS_REF_PREFIX}issues/{id}")
}

/// Returns the ref name for comments on a specific review (pull request).
#[must_use]
pub fn review_comments_ref(id: u64) -> String {
    format!("{COMMENTS_REF_PREFIX}reviews/{id}")
}

/// The location within a Git object that a comment targets.
///
/// The variant encodes the object kind, making invalid combinations
/// (e.g. a line range on a commit) unrepresentable.
#[derive(Clone, Debug)]
pub enum Anchor {
    /// A blob (file), with an optional line range.
    Blob {
        /// SHA of the blob object.
        oid: ::git2::Oid,
        /// Line range `(start, end)` within the blob, if applicable.
        line_range: Option<(u32, u32)>,
    },
    /// A single commit.
    Commit {
        /// SHA of the commit object.
        oid: ::git2::Oid,
    },
    /// A tree (directory).
    Tree {
        /// SHA of the tree object.
        oid: ::git2::Oid,
    },
    /// A range between two commits (inclusive).
    CommitRange {
        /// SHA of the first commit in the range.
        start: ::git2::Oid,
        /// SHA of the last commit in the range.
        end: ::git2::Oid,
    },
}

/// A comment stored as a commit under `refs/forge/comments/`.
///
/// Author identity and timestamp are read from the commit's author field directly.
#[derive(Clone, Debug)]
pub struct Comment {
    /// OID of the commit that represents this comment.
    pub oid: ::git2::Oid,
    /// What this comment is anchored to.
    pub anchor: Anchor,
    /// Markdown body (the commit message, trailers stripped).
    pub body: String,
    /// Whether the thread has been resolved (`Resolved: true` trailer).
    pub resolved: bool,
    /// OID of the parent comment (second parent), if this is a reply.
    pub parent_oid: Option<::git2::Oid>,
    /// OID of a suggestion blob in the commit tree, if present.
    pub suggestion_oid: Option<::git2::Oid>,
}

/// Parameters for appending a new commit to a comment chain.
///
/// Author and timestamp come from the git environment (`GIT_AUTHOR_*` or config).
#[derive(Clone, Debug)]
pub enum NewComment {
    /// A top-level comment anchored to a Git object.
    TopLevel {
        /// What this comment is anchored to.
        anchor: Anchor,
        /// Markdown body.
        body: String,
        /// Optional suggestion blob OID.
        suggestion_oid: Option<::git2::Oid>,
    },
    /// A reply to an existing comment (adds a second parent).
    Reply {
        /// Markdown body.
        body: String,
        /// OID of the comment being replied to (becomes the second parent).
        parent_oid: ::git2::Oid,
        /// Optional suggestion blob OID.
        suggestion_oid: Option<::git2::Oid>,
    },
    /// Resolves a comment thread (adds `Resolved: true` trailer).
    Resolve {
        /// OID of the comment being resolved (becomes the second parent).
        comment_oid: ::git2::Oid,
    },
}

/// Operations on comment refs under [`COMMENTS_REF_PREFIX`].
pub trait Comments {
    /// Return all comments on the given ref, ordered by timestamp ascending.
    ///
    /// # Errors
    ///
    /// Returns `git2::Error` if the underlying repository operation fails.
    fn comments_on(&self, ref_name: &str) -> Result<Vec<Comment>, ::git2::Error>;

    /// Find a single comment by OID on the given ref, returning `None` if not found.
    ///
    /// # Errors
    ///
    /// Returns `git2::Error` if the underlying repository operation fails.
    fn find_comment(
        &self,
        ref_name: &str,
        oid: ::git2::Oid,
    ) -> Result<Option<Comment>, ::git2::Error>;

    /// Append a comment to the chain, returning the OID of the created commit.
    ///
    /// # Errors
    ///
    /// Returns `git2::Error` if the underlying repository operation fails.
    fn add_comment(
        &self,
        ref_name: &str,
        comment: &NewComment,
    ) -> Result<::git2::Oid, ::git2::Error>;
}
