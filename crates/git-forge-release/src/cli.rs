//! CLI definitions for `git forge release`.

use clap::Subcommand;

/// Subcommands for `git forge release`.
#[derive(Subcommand, Clone)]
pub enum ReleaseCommand {
    /// Prepare a new release (stage changelog, bump version, etc.).
    Prepare,
    /// Publish a prepared release.
    Publish,
    /// List releases.
    List,
    /// Show details of a release.
    Show {
        /// Release version tag (e.g. `v1.2.3`).
        version: String,
    },
}
