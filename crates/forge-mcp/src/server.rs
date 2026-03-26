//! MCP server definition.

use rmcp::model::{ServerCapabilities, ServerInfo};
use rmcp::{ServerHandler, tool_handler, tool_router};

/// MCP server that reads forge metadata from a Git repository.
#[derive(Debug, Clone)]
pub struct ForgeServer {
    tool_router: rmcp::handler::server::router::tool::ToolRouter<Self>,
}

impl Default for ForgeServer {
    fn default() -> Self {
        Self::new()
    }
}

impl ForgeServer {
    /// Create a new server instance.
    #[must_use]
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }
}

#[tool_router]
impl ForgeServer {}

#[tool_handler]
impl ServerHandler for ForgeServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
    }
}
