//! MCP tool definitions for forge comments (v2 API).

use rmcp::handler::server::wrapper::Parameters;
use rmcp::{tool, tool_router};
use schemars::JsonSchema;
use serde::Deserialize;

use git_forge::Store;
use git_forge::comment::{
    Anchor, create_thread, edit_in_thread, find_threads_by_object, list_comments_in,
    list_thread_comments, reply_to_thread, resolve_thread,
};

use crate::server::ForgeMcpServer;
use crate::validate::{validate_oid, validate_uuid};

/// Resolve an actor handle to a git2 signature, or return an error string.
fn actor_sig_for(
    server: &ForgeMcpServer,
    handle: &str,
) -> Result<git2::Signature<'static>, String> {
    let repo = server.open_repo()?;
    let store = Store::new(&repo);
    let actor_id = store
        .resolve_actor_handle(handle)
        .map_err(|e| e.to_string())?;
    let actor = store
        .get_actor(actor_id.as_str())
        .map_err(|e| e.to_string())?;
    let name = actor
        .names
        .first()
        .ok_or_else(|| format!("actor {handle} has no names registered"))?
        .clone();
    let email = actor.emails.first().cloned().unwrap_or_default();
    git2::Signature::now(&name, &email).map_err(|e| e.to_string())
}

/// Parameters for the `list_comments_on` tool.
#[derive(Deserialize, JsonSchema)]
struct ListCommentsOnParams {
    /// Git object OID (blob, commit, tree) to list comments on.
    oid: String,
}

/// Parameters for the `list_comments_in` tool.
#[derive(Deserialize, JsonSchema)]
struct ListCommentsInParams {
    /// Commit rev whose reachable tree to search. Defaults to `HEAD`.
    #[serde(default = "default_head")]
    rev: String,
}

fn default_head() -> String {
    "HEAD".to_string()
}

/// Parameters for the `create_comment` tool.
#[derive(Deserialize, JsonSchema)]
struct CreateCommentParams {
    /// Comment body (Markdown).
    body: String,
    /// Git object OID to anchor the comment to.
    anchor_oid: String,
    /// Optional start line (1-based).
    start_line: Option<u32>,
    /// Optional end line (1-based).
    end_line: Option<u32>,
    /// Actor handle to author the comment as (e.g. `"claude"`). Omit to use
    /// the repository's default committer identity.
    author: Option<String>,
}

/// Parameters for the `reply_comment` tool.
#[derive(Deserialize, JsonSchema)]
struct ReplyCommentParams {
    /// Thread UUID.
    thread_id: String,
    /// OID of the comment to reply to.
    reply_to_oid: String,
    /// Reply body (Markdown).
    body: String,
    /// Actor handle to author the reply as. Omit to use the default identity.
    author: Option<String>,
}

/// Parameters for the `resolve_comment` tool.
#[derive(Deserialize, JsonSchema)]
struct ResolveCommentParams {
    /// Thread UUID.
    thread_id: String,
    /// OID of the comment that roots the thread.
    comment_oid: String,
    /// Optional resolution message.
    message: Option<String>,
    /// Actor handle to author the resolution as. Omit to use the default identity.
    author: Option<String>,
}

/// Parameters for the `show_thread` tool.
#[derive(Deserialize, JsonSchema)]
struct ShowThreadParams {
    /// Thread UUID.
    thread_id: String,
}

/// Parameters for the `edit_comment_in_thread` tool.
#[derive(Deserialize, JsonSchema)]
struct EditCommentParams {
    /// Thread UUID.
    thread_id: String,
    /// OID of the comment to edit.
    comment_oid: String,
    /// New body (Markdown).
    body: String,
}

#[tool_router(router = comment_router, vis = "pub(crate)")]
impl ForgeMcpServer {
    /// List all comment threads anchored to a git object (blob, commit, or tree).
    #[tool(description = "List all comments anchored to a git object OID.")]
    fn list_comments_on(
        &self,
        Parameters(params): Parameters<ListCommentsOnParams>,
    ) -> Result<String, String> {
        validate_oid(&params.oid)?;
        let repo = self.open_repo()?;
        let thread_ids = find_threads_by_object(&repo, &params.oid).map_err(|e| e.to_string())?;
        let mut comments = Vec::new();
        for tid in &thread_ids {
            let cs = list_thread_comments(&repo, tid).map_err(|e| e.to_string())?;
            comments.extend(cs);
        }
        comments.sort_by_key(|c| c.timestamp);
        facet_json::to_string_pretty(&comments).map_err(|e| e.to_string())
    }

    /// List all active comments anchored to any object reachable from a commit.
    ///
    /// Walks the commit's full tree (blobs, subtrees, and the commit itself) and
    /// aggregates every comment found.  Use this to see all comments relevant to
    /// the current checkout without needing to know individual OIDs.
    #[tool(
        description = "List all comments reachable from a commit (default: HEAD). \
                       Walks the full tree so agents can see every comment in context."
    )]
    fn list_comments_in(
        &self,
        Parameters(params): Parameters<ListCommentsInParams>,
    ) -> Result<String, String> {
        let repo = self.open_repo()?;
        let comments = list_comments_in(&repo, &params.rev).map_err(|e| e.to_string())?;
        facet_json::to_string_pretty(&comments).map_err(|e| e.to_string())
    }

    /// Create a new comment thread anchored to a git object.
    #[tool(description = "Create a new comment thread anchored to a git object OID.")]
    fn create_comment(
        &self,
        Parameters(params): Parameters<CreateCommentParams>,
    ) -> Result<String, String> {
        validate_oid(&params.anchor_oid)?;
        let sig = params
            .author
            .as_deref()
            .map(|h| actor_sig_for(self, h))
            .transpose()?;
        let repo = self.open_repo()?;
        let anchor = Anchor {
            oid: params.anchor_oid,
            start_line: params.start_line,
            end_line: params.end_line,
        };
        match create_thread(&repo, &params.body, Some(&anchor), None, sig.as_ref()) {
            Ok((thread_id, comment)) => {
                let comment_json =
                    facet_json::to_string_pretty(&comment).map_err(|e| e.to_string())?;
                Ok(format!(
                    "{{\"thread_id\":{thread_id:?},\"comment\":{comment_json}}}"
                ))
            }
            Err(e) => Err(e.to_string()),
        }
    }

    /// Reply to an existing comment thread.
    #[tool(description = "Append a reply to an existing comment thread.")]
    fn reply_comment(
        &self,
        Parameters(params): Parameters<ReplyCommentParams>,
    ) -> Result<String, String> {
        validate_uuid(&params.thread_id)?;
        validate_oid(&params.reply_to_oid)?;
        let sig = params
            .author
            .as_deref()
            .map(|h| actor_sig_for(self, h))
            .transpose()?;
        let repo = self.open_repo()?;
        match reply_to_thread(
            &repo,
            &params.thread_id,
            &params.body,
            &params.reply_to_oid,
            None,
            None,
            sig.as_ref(),
        ) {
            Ok(comment) => facet_json::to_string_pretty(&comment).map_err(|e| e.to_string()),
            Err(e) => Err(e.to_string()),
        }
    }

    /// Resolve a comment thread.
    #[tool(description = "Resolve a comment thread.")]
    fn resolve_comment(
        &self,
        Parameters(params): Parameters<ResolveCommentParams>,
    ) -> Result<String, String> {
        validate_uuid(&params.thread_id)?;
        validate_oid(&params.comment_oid)?;
        let sig = params
            .author
            .as_deref()
            .map(|h| actor_sig_for(self, h))
            .transpose()?;
        let repo = self.open_repo()?;
        match resolve_thread(
            &repo,
            &params.thread_id,
            &params.comment_oid,
            params.message.as_deref(),
            sig.as_ref(),
        ) {
            Ok(comment) => facet_json::to_string_pretty(&comment).map_err(|e| e.to_string()),
            Err(e) => Err(e.to_string()),
        }
    }

    /// Show all comments in a thread.
    #[tool(description = "Show all comments in a thread, identified by thread UUID.")]
    fn show_thread(
        &self,
        Parameters(params): Parameters<ShowThreadParams>,
    ) -> Result<String, String> {
        validate_uuid(&params.thread_id)?;
        let repo = self.open_repo()?;
        match list_thread_comments(&repo, &params.thread_id) {
            Ok(comments) => facet_json::to_string_pretty(&comments).map_err(|e| e.to_string()),
            Err(e) => Err(e.to_string()),
        }
    }

    /// Edit a comment in a thread.
    #[tool(description = "Edit a comment in a thread.")]
    fn edit_comment_in_thread(
        &self,
        Parameters(params): Parameters<EditCommentParams>,
    ) -> Result<String, String> {
        validate_uuid(&params.thread_id)?;
        validate_oid(&params.comment_oid)?;
        let repo = self.open_repo()?;
        match edit_in_thread(
            &repo,
            &params.thread_id,
            &params.comment_oid,
            &params.body,
            None,
            None,
            None,
        ) {
            Ok(comment) => facet_json::to_string_pretty(&comment).map_err(|e| e.to_string()),
            Err(e) => Err(e.to_string()),
        }
    }
}
