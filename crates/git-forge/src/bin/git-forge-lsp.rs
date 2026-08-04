#[path = "../lsp.rs"]
mod lsp;

use std::path::PathBuf;

use anyhow::{Context, Result};
use lsp::Backend;
use tokio::io::{stdin, stdout};
use tower_lsp::{LspService, Server};

#[tokio::main]
async fn main() -> Result<()> {
    let repo = gix::discover(".").context("not inside a git repository")?;
    let root = repo
        .workdir()
        .map(PathBuf::from)
        .unwrap_or_else(|| repo.common_dir().to_path_buf());
    let (service, socket) = LspService::new(|_| Backend::new(root));
    Server::new(stdin(), stdout(), socket).serve(service).await;
    Ok(())
}
