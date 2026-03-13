//! The CLI definitions for the top-level `git forge` command.

pub mod check;
pub mod issue;
pub mod release;
pub mod review;

use clap::{Parser, Subcommand};

/// Local-first infrastructure for Git forges.
#[derive(Parser)]
#[command(name = "git forge", bin_name = "git forge")]
#[command(author, version)]
pub struct Cli {
    /// The subcommand to run.
    #[command(subcommand)]
    pub command: Commands,
}

/// Top-level subcommands.
#[derive(Subcommand)]
pub enum Commands {
    /// Work with issues.
    Issue {
        /// The issue subcommand to run.
        #[command(subcommand)]
        command: issue::IssueCommand,
    },
    /// Work with pull/merge request reviews.
    Review {
        /// The review subcommand to run.
        #[command(subcommand)]
        command: review::ReviewCommand,
    },
    /// Work with CI checks.
    Check {
        /// The check subcommand to run.
        #[command(subcommand)]
        command: check::CheckCommand,
    },
    /// Work with releases.
    Release {
        /// The release subcommand to run.
        #[command(subcommand)]
        command: release::ReleaseCommand,
    },
}
