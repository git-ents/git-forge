//! Execution logic for `git forge review`.

use crate::cli::ReviewCommand;

/// Execute a `review` subcommand.
pub fn run(command: ReviewCommand, _push: bool) {
    match command {
        ReviewCommand::New => todo!(),
        ReviewCommand::Edit => todo!(),
        ReviewCommand::List => todo!(),
        ReviewCommand::Status => todo!(),
        ReviewCommand::Show => todo!(),
    }
}
