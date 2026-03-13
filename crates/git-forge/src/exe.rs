//! Execution logic for `git forge` subcommands.

use crate::cli::check::CheckCommand;
use crate::cli::issue::IssueCommand;
use crate::cli::release::ReleaseCommand;
use crate::cli::review::ReviewCommand;

pub mod issue {
    //! Execution logic for `git forge issue`.

    use super::IssueCommand;

    /// Execute an `issue` subcommand.
    pub fn run(command: IssueCommand) {
        match command {
            IssueCommand::New => todo!(),
            IssueCommand::Edit => todo!(),
            IssueCommand::List => todo!(),
            IssueCommand::Status => todo!(),
            IssueCommand::Show => todo!(),
        }
    }
}

pub mod review {
    //! Execution logic for `git forge review`.

    use super::ReviewCommand;

    /// Execute a `review` subcommand.
    pub fn run(command: ReviewCommand) {
        match command {
            ReviewCommand::New => todo!(),
            ReviewCommand::Edit => todo!(),
            ReviewCommand::List => todo!(),
            ReviewCommand::Status => todo!(),
            ReviewCommand::Show => todo!(),
        }
    }
}

pub mod check {
    //! Execution logic for `git forge check`.

    use super::CheckCommand;

    /// Execute a `check` subcommand.
    pub fn run(command: CheckCommand) {
        match command {
            CheckCommand::New => todo!(),
            CheckCommand::Edit => todo!(),
            CheckCommand::List => todo!(),
            CheckCommand::Status => todo!(),
            CheckCommand::Show => todo!(),
        }
    }
}

pub mod release {
    //! Execution logic for `git forge release`.

    use super::ReleaseCommand;

    /// Execute a `release` subcommand.
    pub fn run(command: ReleaseCommand) {
        match command {
            ReleaseCommand::New => todo!(),
            ReleaseCommand::Edit => todo!(),
            ReleaseCommand::List => todo!(),
            ReleaseCommand::Status => todo!(),
            ReleaseCommand::Show => todo!(),
        }
    }
}
