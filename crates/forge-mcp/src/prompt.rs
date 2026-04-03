//! MCP prompt definitions for forge issues.
//!
//! These prompts surface as slash commands in editors like Zed.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{PromptMessage, PromptMessageRole};
use rmcp::{prompt, prompt_router};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::server::ForgeMcpServer;

/// Parameters for the `list_issues` prompt.
#[derive(Deserialize, JsonSchema)]
struct ListIssuesArgs {
    /// Filter by state: `"open"` or `"closed"`. Omit to return all issues.
    state: Option<String>,
}

/// Parameters for the `get_issue` prompt.
#[derive(Deserialize, JsonSchema)]
struct GetIssueArgs {
    /// Display ID (e.g. `"3"`, `"GH1"`) or OID prefix.
    reference: String,
}

#[prompt_router(vis = "pub(crate)")]
#[allow(missing_docs)]
impl ForgeMcpServer {
    /// List issues in the forge repository.
    #[prompt(
        name = "list-issues",
        description = "List issues in the forge repository"
    )]
    fn list_issues_prompt(
        &self,
        Parameters(args): Parameters<ListIssuesArgs>,
    ) -> Result<Vec<PromptMessage>, rmcp::ErrorData> {
        let state = args.state.filter(|s| !s.is_empty());
        let text = self
            .fetch_issues(state.as_deref())
            .map_err(|e| rmcp::ErrorData::internal_error(e, None))?;
        Ok(vec![PromptMessage::new_text(PromptMessageRole::User, text)])
    }

    /// Get a single issue by display ID or OID prefix.
    #[prompt(
        name = "get-issue",
        description = "Get a single forge issue by display ID or OID prefix"
    )]
    fn get_issue_prompt(
        &self,
        Parameters(args): Parameters<GetIssueArgs>,
    ) -> Result<Vec<PromptMessage>, rmcp::ErrorData> {
        let text = self
            .fetch_issue(&args.reference)
            .map_err(|e| rmcp::ErrorData::internal_error(e, None))?;
        Ok(vec![PromptMessage::new_text(PromptMessageRole::User, text)])
    }
}
