//! CLI definitions for `git forge review`.

use clap::Subcommand;

/// Subcommands for `git forge review`.
#[derive(Subcommand, Clone)]
pub enum ReviewCommand {
    /// Open a new review.
    New {
        /// Target branch. Defaults to the repository's default branch.
        #[arg(long)]
        target: Option<String>,
    },

    /// Show details of a review. Defaults to the current branch's review.
    Show {
        /// Review ID. Defaults to current branch's active review.
        id: Option<String>,

        /// One-line summary.
        #[arg(long)]
        oneline: bool,
    },

    /// List reviews.
    List {
        /// Filter by state.
        #[arg(long, value_enum, default_value_t = ReviewStateArg::Open)]
        state: ReviewStateArg,
    },

    /// Edit a review's title or description.
    Edit {
        /// Review ID. Defaults to current branch's active review.
        id: Option<String>,

        /// New title.
        #[arg(long)]
        title: Option<String>,

        /// New description.
        #[arg(long)]
        description: Option<String>,
    },

    /// Approve a review.
    Approve {
        /// Review ID. Defaults to current branch's active review.
        id: Option<String>,
    },

    /// Reject a review (request changes).
    Reject {
        /// Review ID. Defaults to current branch's active review.
        id: Option<String>,
    },

    /// Merge a review.
    Merge {
        /// Review ID. Defaults to current branch's active review.
        id: Option<String>,
    },

    /// Close a review without merging.
    Close {
        /// Review ID. Defaults to current branch's active review.
        id: Option<String>,
    },

    /// Add a comment to a review.
    Comment {
        /// Review ID. Defaults to current branch's active review.
        id: Option<String>,

        /// Comment body (markdown). Opens an editor when omitted in an interactive shell.
        body: Option<String>,
    },
}

/// Review lifecycle state, as a CLI argument.
#[derive(Clone, Copy, Debug, clap::ValueEnum)]
pub enum ReviewStateArg {
    /// The review is open.
    Open,
    /// The review was merged.
    Merged,
    /// The review was closed without merging.
    Closed,
}
