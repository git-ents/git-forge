//! CLI definitions for `git forge comment`.

use clap::Subcommand;

/// Subcommands for `git forge comment`.
#[derive(Subcommand, Clone)]
pub enum CommentCommand {
    /// Add a comment to an issue or review.
    Add {
        /// Target: "issue/<id>" or "review/<id>".
        target: String,

        /// Comment body (markdown). Reads from stdin if omitted.
        #[arg(short, long)]
        body: Option<String>,

        /// Blob SHA being commented on.
        #[arg(long)]
        anchor: Option<String>,

        /// Anchor type: blob, commit, tree, or commit-range.
        #[arg(long)]
        anchor_type: Option<String>,

        /// Line range, e.g. "42-47" (blob anchors only).
        #[arg(long)]
        range: Option<String>,
    },

    /// Reply to an existing comment.
    Reply {
        /// Target: "issue/<id>" or "review/<id>".
        target: String,

        /// OID of the comment to reply to.
        comment: String,

        /// Reply body (markdown). Reads from stdin if omitted.
        #[arg(short, long)]
        body: Option<String>,
    },

    /// Resolve a comment thread.
    Resolve {
        /// Target: "issue/<id>" or "review/<id>".
        target: String,

        /// OID of the comment to resolve.
        comment: String,

        /// Optional resolution message.
        #[arg(short, long)]
        message: Option<String>,
    },

    /// List comments.
    List {
        /// Target: "issue/<id>" or "review/<id>".
        target: String,
    },
}
