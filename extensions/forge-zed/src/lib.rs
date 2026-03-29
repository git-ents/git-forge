use zed_extension_api::{self as zed, ContextServerId, Project, Command, Result};

struct ForgeExtension;

impl zed::Extension for ForgeExtension {
    fn new() -> Self {
        Self
    }

    fn context_server_command(
        &mut self,
        _context_server_id: &ContextServerId,
        _project: &Project,
    ) -> Result<Command> {
        Ok(Command {
            command: "forge-mcp".to_string(),
            args: vec![],
            env: vec![],
        })
    }
}

zed::register_extension!(ForgeExtension);
