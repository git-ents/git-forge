//! Execution logic for `git forge release`.

use crate::cli::ReleaseCommand;

/// Execute a `release` subcommand.
pub fn run(command: ReleaseCommand, _push: bool, _fetch: bool) {
    match command {
        ReleaseCommand::New => todo!(),
        ReleaseCommand::Edit => todo!(),
        ReleaseCommand::List => todo!(),
        ReleaseCommand::Status => todo!(),
        ReleaseCommand::Show => todo!(),
    }
}
