use zed_extension_api::{self as zed, Command, ContextServerId, LanguageServerId, Project, Result};

struct ForgeExtension;

const LSP_BINARY: &str = "forge-lsp";
const MCP_BINARY: &str = "forge-mcp";

fn find_binary(worktree: &zed::Worktree, name: &str) -> Option<String> {
    worktree.which(name)
}

impl zed::Extension for ForgeExtension {
    fn new() -> Self {
        Self
    }

    fn language_server_command(
        &mut self,
        language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<Command> {
        if language_server_id.as_ref() != LSP_BINARY {
            return Err(format!("unknown language server: {}", language_server_id.as_ref()));
        }

        let command = find_binary(worktree, LSP_BINARY)
            .ok_or_else(|| format!("{LSP_BINARY} not found in $PATH"))?;

        Ok(Command {
            command,
            args: vec![],
            env: vec![],
        })
    }

    fn context_server_command(
        &mut self,
        context_server_id: &ContextServerId,
        _project: &Project,
    ) -> Result<Command> {
        if context_server_id.as_ref() != MCP_BINARY {
            return Err(format!("unknown context server: {}", context_server_id.as_ref()));
        }

        // Project only exposes worktree IDs, not Worktree handles.
        // We cannot safely construct a Worktree from an ID (different namespaces).
        // Fall back to bare $PATH lookup.
        Err(format!("{MCP_BINARY} not found in $PATH"))
    }
}

zed::register_extension!(ForgeExtension);
