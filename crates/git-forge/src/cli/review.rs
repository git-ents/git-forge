//! CLI definitions for `git forge release`.

use clap::Subcommand;

/// Commands for `git forge review`.
#[derive(Subcommand)]
pub enum ReviewCommand {
    /// Open a new review.
    New,
    /// Edit an existing review.
    Edit,
    /// List reviews.
    List,
    /// Show the status of a review.
    Status,
    /// Show details of a review.
    Show,
}
