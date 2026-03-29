//! MCP prompt definitions for forge issues.
//!
//! These prompts surface as slash commands in editors like Zed.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{PromptMessage, PromptMessageRole};
use rmcp::{prompt, prompt_router};
use schemars::JsonSchema;
use serde::Deserialize;

use git_forge::Store;
use git_forge::issue::IssueState;

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
        let repo = self
            .open_repo()
            .map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))?;
        let store = Store::new(&repo);
        let issues = match args.state.as_deref() {
            None => store.list_issues(),
            Some(s) => s.parse::<IssueState>().map_or_else(
                |_| Err(git_forge::Error::InvalidState(s.to_string())),
                |state| store.list_issues_by_state(&state),
            ),
        };
        let text = match issues {
            Ok(list) => facet_json::to_string_pretty(&list).expect("serialize"),
            Err(e) => format!("error: {e}"),
        };
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
        let repo = self
            .open_repo()
            .map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))?;
        let store = Store::new(&repo);
        let text = match store.get_issue(&args.reference) {
            Ok(issue) => facet_json::to_string_pretty(&issue).expect("serialize"),
            Err(e) => format!("error: {e}"),
        };
        Ok(vec![PromptMessage::new_text(PromptMessageRole::User, text)])
    }
}
