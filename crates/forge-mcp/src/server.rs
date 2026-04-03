//! MCP server struct and transport wiring.

use std::path::PathBuf;

use git2::Repository;
use rmcp::RoleServer;
use rmcp::handler::server::router::prompt::PromptRouter;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::model::{
    GetPromptRequestParams, GetPromptResult, ListPromptsResult, PaginatedRequestParams,
    ServerCapabilities, ServerInfo,
};
use rmcp::service::RequestContext;
use rmcp::{ServerHandler, prompt_handler, tool_handler};

use git_forge::Store;
use git_forge::issue::IssueState;

/// MCP server that exposes forge metadata from a Git repository.
///
/// The `Repository` handle is opened per-call via [`Self::open_repo`] because
/// `git2::Repository` is `!Sync`, so it cannot be held across the async
/// boundary in the MCP handler. The repo *path* is cached and reused.
#[derive(Debug, Clone)]
pub struct ForgeMcpServer {
    repo_path: PathBuf,
    pub(crate) tool_router: ToolRouter<Self>,
    pub(crate) prompt_router: PromptRouter<Self>,
}

impl ForgeMcpServer {
    /// Discover the forge Git repository.
    ///
    /// Uses `FORGE_REPO_PATH` if set, otherwise discovers from the current
    /// directory.
    ///
    /// # Errors
    /// Returns an error if no repository is found.
    pub fn new() -> anyhow::Result<Self> {
        let repo = match std::env::var("FORGE_REPO_PATH") {
            Ok(path) => Repository::discover(&path)?,
            Err(_) => Repository::discover(".")?,
        };
        let repo_path = repo.path().to_path_buf();
        let mut tool_router = Self::issue_router();
        tool_router.merge(Self::comment_router());
        tool_router.merge(Self::review_router());
        Ok(Self {
            repo_path,
            tool_router,
            prompt_router: Self::prompt_router(),
        })
    }

    pub(crate) fn open_repo(&self) -> Result<Repository, String> {
        Repository::open(&self.repo_path).map_err(|e| e.to_string())
    }

    pub(crate) fn fetch_issues(&self, state: Option<&str>) -> Result<String, String> {
        let repo = self.open_repo()?;
        let store = Store::new(&repo);
        let issues = match state {
            None => store.list_issues(),
            Some(s) => s.parse::<IssueState>().map_or_else(
                |_| Err(git_forge::Error::InvalidState(s.to_string())),
                |st| store.list_issues_by_state(&st),
            ),
        };
        match issues {
            Ok(list) => facet_json::to_string_pretty(&list).map_err(|e| e.to_string()),
            Err(e) => Err(e.to_string()),
        }
    }

    pub(crate) fn fetch_issue(&self, reference: &str) -> Result<String, String> {
        let repo = self.open_repo()?;
        let store = Store::new(&repo);
        match store.get_issue(reference) {
            Ok(issue) => facet_json::to_string_pretty(&issue).map_err(|e| e.to_string()),
            Err(e) => Err(e.to_string()),
        }
    }

    /// Construct a server pointing at an explicit repository path, for use in tests.
    #[cfg(test)]
    pub(crate) fn for_test(repo_path: std::path::PathBuf) -> Self {
        let mut tool_router = Self::issue_router();
        tool_router.merge(Self::comment_router());
        tool_router.merge(Self::review_router());
        Self {
            repo_path,
            tool_router,
            prompt_router: Self::prompt_router(),
        }
    }
}

#[tool_handler]
#[prompt_handler]
impl ServerHandler for ForgeMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_prompts()
                .build(),
        )
    }
}
