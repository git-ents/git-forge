//! Release workflows and release refs.
//!
//! A release is a repository maintenance workflow, not a build step. Given the
//! commits since the last release, Forge determines the version bump, applies
//! version changes, and generates the changelog.
//!
//! - [`git2`] — `git2::Repository` implementation of [`Releases`].
//!
//! ## Release refs
//!
//! ```text
//! refs/tags/v1.2.0              → signed commit
//! refs/meta/releases/v1.2.0    → tree
//! ├── meta                      # toml: version, date, author
//! ├── changelog                 # markdown
//! └── artifacts/
//!     ├── x86_64-linux/
//!     └── aarch64-darwin/
//! ```
//!
//! ## Workflow
//!
//! `prepare` parses conventional commits since the last tag, determines the
//! version bump, runs version updaters, generates the changelog, creates a
//! release branch, and opens a review ref for the release.
//!
//! `publish` tags the merged commit, attaches build artifacts to the release
//! ref, and pushes.

pub mod git2;

/// Ref prefix under which release metadata trees are stored.
pub const RELEASES_REF_PREFIX: &str = "refs/meta/releases/";

/// The type of version bump determined from conventional commits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum BumpKind {
    /// A backwards-compatible bug fix (`fix:` commits).
    Patch,
    /// A backwards-compatible new feature (`feat:` commits).
    Minor,
    /// A breaking change (`BREAKING CHANGE` footer or `!` suffix).
    Major,
}

impl BumpKind {
    /// Canonical string representation.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Patch => "patch",
            Self::Minor => "minor",
            Self::Major => "major",
        }
    }
}

/// Metadata stored in a release's `meta` file.
#[derive(Clone, Debug)]
pub struct ReleaseMeta {
    /// The version string (e.g. `"1.2.0"`).
    pub version: String,
    /// RFC 3339 date of publication.
    pub date: String,
    /// Fingerprint of the contributor who published the release.
    pub author: String,
}

/// A build artifact attached to a release.
#[derive(Clone, Debug)]
pub struct Artifact {
    /// The platform subdirectory name (e.g. `"x86_64-linux"`).
    pub platform: String,
    /// Raw bytes of the artifact blob.
    pub data: Vec<u8>,
}

/// A fully loaded release.
#[derive(Clone, Debug)]
pub struct Release {
    /// The version string (e.g. `"1.2.0"`).
    pub version: String,
    /// Metadata from the `meta` file.
    pub meta: ReleaseMeta,
    /// Markdown changelog generated for this release.
    pub changelog: String,
    /// Build artifacts attached to this release.
    pub artifacts: Vec<Artifact>,
}

/// The result of the `prepare` step — all inputs needed to create the release
/// branch and open a review.
#[derive(Clone, Debug)]
pub struct ReleasePlan {
    /// The version that will be released.
    pub version: String,
    /// The determined bump kind.
    pub bump: BumpKind,
    /// The generated markdown changelog.
    pub changelog: String,
    /// The commit range included in this release (`base_tag..HEAD`).
    pub commit_range: (::git2::Oid, ::git2::Oid),
}

/// Parameters for attaching a new artifact to a release.
#[derive(Clone, Debug)]
pub struct NewArtifact {
    /// The platform identifier (e.g. `"aarch64-darwin"`).
    pub platform: String,
    /// Raw artifact bytes.
    pub data: Vec<u8>,
}

/// Operations on release refs and release workflows.
pub trait Releases {
    /// Return the release metadata ref name for a version string.
    fn release_ref(version: &str) -> String {
        format!("{RELEASES_REF_PREFIX}{version}")
    }

    /// Return the tag ref name for a version string.
    fn release_tag(version: &str) -> String {
        format!("refs/tags/v{version}")
    }

    /// Return all releases, ordered by version descending (most recent first).
    fn list_releases(&self) -> Result<Vec<Release>, ::git2::Error>;

    /// Load the release for `version`, returning `None` if the ref does not
    /// exist.
    fn find_release(&self, version: &str) -> Result<Option<Release>, ::git2::Error>;

    /// Analyse commits since the last release tag and produce a
    /// [`ReleasePlan`] without writing anything to the repository.
    ///
    /// Returns `None` when there are no releasable commits since the last tag
    /// (i.e. no `fix:`, `feat:`, or breaking-change commits).
    fn prepare_release(&self) -> Result<Option<ReleasePlan>, ::git2::Error>;

    /// Apply `plan` to the repository: create a release branch, update version
    /// files, commit the changelog, and open a review ref targeting
    /// `target_branch`.
    ///
    /// Returns the review ID of the newly opened release review.
    fn apply_release_plan(
        &self,
        plan: &ReleasePlan,
        target_branch: &str,
    ) -> Result<u64, ::git2::Error>;

    /// Publish a release: create a signed tag at `commit_oid`, write the
    /// release metadata ref, and attach any provided artifacts.
    ///
    /// `commit_oid` should be the merged release commit on the target branch.
    fn publish_release(
        &self,
        version: &str,
        commit_oid: ::git2::Oid,
        changelog: &str,
        artifacts: &[NewArtifact],
    ) -> Result<(), ::git2::Error>;

    /// Attach an additional artifact to an already-published release.
    fn attach_artifact(&self, version: &str, artifact: &NewArtifact) -> Result<(), ::git2::Error>;
}
