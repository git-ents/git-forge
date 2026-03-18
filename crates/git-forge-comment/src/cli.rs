//! CLI definitions for `git forge comment`.

use clap::Subcommand;

/// Subcommands for `git forge comment`.
#[derive(Subcommand, Clone)]
pub enum CommentCommand {
    /// Add a new comment to any git object.
    New {
        /// Target: "issue/<id>", "review/<id>", "commit/<sha>", "blob/<sha>", etc. Defaults to "commit/<HEAD>".
        target: Option<String>,

        /// Comment body (markdown). Opens an editor when omitted in an interactive shell.
        #[arg(short, long)]
        body: Option<String>,

        /// Object SHA being commented on (defaults to HEAD commit).
        #[arg(long)]
        anchor: Option<String>,

        /// Anchor type: blob, commit, tree, or commit-range.
        #[arg(long)]
        anchor_type: Option<String>,

        /// Line range within a blob, e.g. "42-47".
        #[arg(long)]
        range: Option<String>,
    },

    /// Reply to an existing comment.
    Reply {
        /// OID of the comment to reply to.
        comment: String,

        /// Reply body (markdown). Opens an editor when omitted in an interactive shell.
        #[arg(short, long)]
        body: Option<String>,
    },

    /// Edit a comment (creates a new immutable comment with Replaces trailer).
    Edit {
        /// OID of the comment to edit.
        comment: String,

        /// Target: "issue/<id>", "review/<id>", "commit/<sha>", etc. Defaults to "commit/<HEAD>".
        target: Option<String>,

        /// New body (markdown). Opens interactive editor if omitted.
        #[arg(short, long)]
        body: Option<String>,
    },

    /// Resolve a comment thread.
    Resolve {
        /// OID of the comment to resolve.
        comment: String,

        /// Optional resolution message.
        #[arg(short, long)]
        message: Option<String>,
    },

    /// List comments on a target.
    List {
        /// Target: "issue/<id>", "review/<id>", "commit/<sha>", etc. Defaults to "commit/<HEAD>".
        target: Option<String>,

        /// Show all comments across all targets.
        #[arg(short = 'a', long)]
        all: bool,
    },

    /// Show comments. Defaults to all active comments on HEAD.
    Show {
        /// OID of a specific comment to show. Defaults to all active comments.
        #[arg(short, long = "ref")]
        comment: Option<String>,

        /// Target: "issue/<id>", "review/<id>", "commit/<sha>", etc. Defaults to "commit/<HEAD>".
        #[arg(short, long)]
        target: Option<String>,

        /// Show reply threads indented under their parent comments.
        #[arg(short = 'T', long)]
        threads: bool,
    },
}
