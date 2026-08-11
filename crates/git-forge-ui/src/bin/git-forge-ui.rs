//! `git-forge-ui`: the web UI for a `git-forge` repository.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use clap::Parser;
use gix_forge::{EntityOps, Member};

#[derive(Parser)]
#[command(name = "git-forge-ui", about = "Serve the git-forge web UI", version)]
struct Cli {
    /// Repository to serve, discovered upwards from this path.
    #[arg(long, default_value = ".", value_name = "PATH")]
    repo: PathBuf,
    /// Serve exactly this Git directory, skipping discovery. The `git-forge`
    /// CLI passes the directory it already resolved so that the UI cannot land
    /// on a different repository than the rest of the command line.
    #[arg(long, value_name = "PATH", conflicts_with = "repo")]
    git_dir: Option<PathBuf>,
    #[arg(long)]
    open: bool,
    #[arg(long, default_value = "5050")]
    port: Option<u16>,
    #[arg(long, default_value = "git-forge.localhost")]
    host: String,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("failed to create the UI runtime")?;

    runtime.block_on(async move {
        let repo = match &cli.git_dir {
            Some(git_dir) => gix::ThreadSafeRepository::open(git_dir)
                .with_context(|| format!("failed to open the repository {}", git_dir.display()))?,
            None => gix::ThreadSafeRepository::discover(&cli.repo)
                .with_context(|| format!("not inside a git repository: {}", cli.repo.display()))?,
        };
        let member_id = resolve_member_id(&repo.to_thread_local());
        let router = git_forge_ui::build_router(Arc::new(repo), member_id).await;
        let listener = tokio::net::TcpListener::bind((cli.host.as_str(), cli.port.unwrap_or(0)))
            .await
            .with_context(|| format!("failed to bind the UI to {}:{:?}", cli.host, cli.port))?;
        let port = listener
            .local_addr()
            .context("failed to determine the UI listener address")?
            .port();

        let url = ui_url(&cli.host, port);
        println!("{url}");
        if cli.open {
            open_url(&url)?;
        }
        topcoat::serve(listener, router)
            .await
            .context("the UI server failed")?;
        Ok(())
    })
}

/// The member whose signing key matches this checkout, when exactly one does.
fn resolve_member_id(repo: &gix::Repository) -> Option<String> {
    let signing_key = effective_signing_key().ok().flatten()?;
    let members = load_members(repo).ok()?;
    let mut matches = members
        .into_iter()
        .filter(|member| member.signing_key == signing_key);
    let member = matches.next()?;
    matches.next().is_none().then_some(member.id)
}

fn load_members(repo: &gix::Repository) -> Result<Vec<Member>> {
    Member::list(repo)?
        .into_iter()
        .map(|id| Member::load_from_repo(repo, &id)?.with_context(|| format!("no member {id}")))
        .collect()
}

fn effective_signing_key() -> Result<Option<String>> {
    let output = std::process::Command::new("git")
        .args(["config", "--get", "user.signingKey"])
        .output()
        .context("failed to read effective Git config user.signingKey")?;
    if !output.status.success() {
        return Ok(None);
    }
    let signing_key = String::from_utf8(output.stdout)
        .context("effective Git config user.signingKey is not valid UTF-8")?;
    Ok((!signing_key.trim().is_empty()).then(|| signing_key.trim().to_owned()))
}

fn ui_url(host: &str, port: u16) -> String {
    let host = if host.starts_with('[') && host.ends_with(']') {
        host.to_owned()
    } else if host.contains(':') {
        format!("[{host}]")
    } else {
        host.to_owned()
    };
    format!("https://{host}:{port}")
}

#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
fn open_url(url: &str) -> Result<()> {
    #[cfg(target_os = "macos")]
    let mut command = std::process::Command::new("open");
    #[cfg(target_os = "linux")]
    let mut command = std::process::Command::new("xdg-open");
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = std::process::Command::new("cmd");
        command.args(["/C", "start", "", url]);
        command
    };
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    command.arg(url);
    let status = command.status().context("failed to open the UI URL")?;
    if !status.success() {
        bail!("failed to open the UI URL: {url}");
    }
    Ok(())
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn open_url(_url: &str) -> Result<()> {
    bail!("opening UI URLs is unsupported on this platform");
}
