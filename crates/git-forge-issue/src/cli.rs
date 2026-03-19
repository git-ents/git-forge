//! CLI definitions for `git forge issue`.

use clap::Subcommand;

/// Subcommands for `git forge issue`.
#[derive(Subcommand, Clone)]
pub enum IssueCommand {
    /// Open a new issue.
    New {
        /// Issue title. Omit to open an editor (in an interactive shell).
        title: Option<String>,

        /// Issue body (markdown). Reads from stdin if omitted in a non-interactive shell.
        #[arg(short, long)]
        body: Option<String>,
    },

    /// Show details of an issue.
    Show {
        /// Issue ID.
        id: u64,

        /// One-line summary.
        #[arg(long)]
        oneline: bool,
    },

    /// List issues.
    List {
        /// Filter by state.
        #[arg(long, value_enum, default_value_t = StateArg::Open)]
        state: StateArg,

        /// Filter by label (repeatable).
        #[arg(long = "label")]
        labels: Vec<String>,

        /// Filter by assignee (repeatable).
        #[arg(long = "assignee")]
        assignees: Vec<String>,
    },

    /// Edit an existing issue.
    Edit {
        /// Issue ID.
        id: u64,

        /// New title.
        #[arg(short, long)]
        title: Option<String>,

        /// New body.
        #[arg(short, long)]
        body: Option<String>,
    },

    /// Close an issue.
    Close {
        /// Issue ID.
        id: u64,
    },

    /// Reopen a closed issue.
    Reopen {
        /// Issue ID.
        id: u64,
    },

    /// Add or remove labels on an issue.
    Label {
        /// Issue ID.
        id: u64,

        /// Label to add (repeatable).
        #[arg(long = "add")]
        add: Vec<String>,

        /// Label to remove (repeatable).
        #[arg(long = "remove")]
        remove: Vec<String>,
    },

    /// Add or remove assignees on an issue.
    Assign {
        /// Issue ID.
        id: u64,

        /// Assignee to add (repeatable).
        #[arg(long = "add")]
        add: Vec<String>,

        /// Assignee to remove (repeatable).
        #[arg(long = "remove")]
        remove: Vec<String>,
    },

    /// Add a comment to an issue.
    Comment {
        /// Issue ID.
        id: u64,

        /// Comment body (markdown). Opens an editor when omitted in an interactive shell.
        body: Option<String>,
    },
}

/// Issue lifecycle state, as a CLI argument.
#[derive(Clone, Copy, Debug, clap::ValueEnum)]
pub enum StateArg {
    /// The issue is active and unresolved.
    Open,
    /// The issue has been resolved or won't be fixed.
    Closed,
    /// All issues regardless of state.
    All,
}
