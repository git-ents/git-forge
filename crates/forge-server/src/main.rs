//! Forge sync daemon — standalone binary entry point.

use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use git2::Repository;

/// Forge sync daemon — watches refs and coordinates GitHub sync.
#[derive(Parser, Debug)]
#[command(version)]
struct Args {
    /// Path to the git repository (default: current directory).
    #[arg(long, default_value = ".")]
    repo: PathBuf,

    /// Seconds between sync polls.
    #[arg(long, default_value_t = 60u64)]
    poll_interval: u64,

    /// Run a single sync pass and exit.
    #[arg(long)]
    once: bool,

    /// Git remote for pushing/pulling forge refs (default: origin).
    #[arg(long, default_value = "origin")]
    remote: String,

    /// Disable fetching/pushing forge refs to the remote.
    #[arg(long)]
    no_sync_refs: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let repo = Repository::discover(&args.repo)?;

    let config = forge_server::ServerConfig {
        poll_interval: args.poll_interval,
        once: args.once,
        remote: args.remote,
        no_sync_refs: args.no_sync_refs,
    };

    forge_server::run(&repo, &config)
}
