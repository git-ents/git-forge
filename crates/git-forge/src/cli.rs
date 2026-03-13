//! The CLI definitions for the top-level `git forge` command.

pub mod issue;

use clap::{Parser, Subcommand};

/// Local-first infrastructure for Git forges.
#[derive(Parser)]
#[command(name = "git forge", bin_name = "git forge")]
#[command(author, version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

/// Top-level subcommands.
#[derive(Subcommand)]
pub enum Commands {
    /// Work with issues.
    Issue {
        #[command(subcommand)]
        verb: issue::IssueVerb,
    },
}
