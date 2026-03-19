//! Execution logic for `git forge review`.

use crate::cli::ReviewCommand;

/// Execute a `review` subcommand.
pub fn run(command: ReviewCommand, _push: bool, _fetch: bool) {
    match command {
        ReviewCommand::New { .. } => todo!(),
        ReviewCommand::Show { .. } => todo!(),
        ReviewCommand::List { .. } => todo!(),
        ReviewCommand::Edit { .. } => todo!(),
        ReviewCommand::Approve { .. } => todo!(),
        ReviewCommand::Reject { .. } => todo!(),
        ReviewCommand::Merge { .. } => todo!(),
        ReviewCommand::Close { .. } => todo!(),
        ReviewCommand::Comment { .. } => todo!(),
    }
}
