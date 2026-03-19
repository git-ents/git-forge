//! Execution logic for `git forge release`.

use crate::cli::ReleaseCommand;

/// Execute a `release` subcommand.
pub fn run(command: ReleaseCommand) {
    match command {
        ReleaseCommand::Prepare => todo!(),
        ReleaseCommand::Publish => todo!(),
        ReleaseCommand::List => todo!(),
        ReleaseCommand::Show { .. } => todo!(),
    }
}
