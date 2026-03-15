//! Execution logic for `git forge issue`.

use std::process;

use git2::Repository;

use crate::cli::{IssueCommand, StateArg};
use crate::issues::{IssueState, Issues};

#[allow(dead_code)]
fn open_repo() -> Repository {
    match Repository::open_from_env() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error: {e}");
            process::exit(1);
        }
    }
}

/// A wrapper type which manipulates issues for the provided repository.
struct Executor(git2::Repository);

impl Executor {
    /// Constructs an `Executor` from a path to a repository.
    #[allow(dead_code)]
    pub fn from_path(path: &str) -> Result<Self, git2::Error> {
        let repo = Repository::open(path)?;
        Ok(Self(repo))
    }

    /// Constructs an `Executor` from [`Repository::open_from_env()`].
    pub fn from_env() -> Result<Self, git2::Error> {
        let repo = Repository::open_from_env()?;
        Ok(Self(repo))
    }

    /// Return a reference the underlying [`git2::Repository`].
    pub fn repo(&self) -> &git2::Repository {
        &self.0
    }

    /// Lists issues for the repository, optionally filtered by state.
    #[allow(dead_code)]
    pub fn list_issues(&self, state: Option<IssueState>) -> Result<(), Box<dyn std::error::Error>> {
        let repo = self.repo();

        let issues = match state {
            Some(state) => repo.list_issues_by_state(state, None)?,
            None => repo.list_issues(None)?,
        };

        for issue in issues {
            println!("#{}\t{}", issue.id, issue.meta.title);
        }
        Ok(())
    }

    /// Creates a new issue with the given title and body.
    pub fn create_issue(
        &self,
        title: &str,
        body: &str,
        label: Option<&[String]>,
        assignee: Option<&[String]>,
    ) -> Result<u64, Box<dyn std::error::Error>> {
        let repo = self.repo();
        let labels = label.unwrap_or_default();
        let assignees = assignee.unwrap_or_default();
        let id = repo.create_issue(title, body, labels, assignees, None)?;
        Ok(id)
    }

    /// Updates an existing issue.
    pub fn edit_issue(
        &self,
        id: u64,
        title: Option<&str>,
        body: Option<&str>,
        labels: Option<&[String]>,
        assignees: Option<&[String]>,
        state: Option<IssueState>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let repo = self.repo();
        repo.update_issue(id, title, body, labels, assignees, state, None)?;
        Ok(())
    }

    /// Displays the full details of an issue.
    pub fn show_issue(&self, id: u64) -> Result<(), Box<dyn std::error::Error>> {
        let repo = self.repo();
        match repo.find_issue(id, None)? {
            None => {
                eprintln!("Issue #{id} not found.");
                process::exit(1);
            }
            Some(issue) => {
                println!("Issue #{}", issue.id);
                println!("Title:  {}", issue.meta.title);
                println!("State:  {}", issue.meta.state.as_str());
                println!("Author: {}", issue.meta.author);
                if !issue.meta.labels.is_empty() {
                    println!("Labels: {}", issue.meta.labels.join(", "));
                }
                println!();
                println!("{}", issue.body);
                if !issue.comments.is_empty() {
                    println!();
                    println!("Comments ({})", issue.comments.len());
                    for (name, body) in &issue.comments {
                        println!("---");
                        println!("{name}");
                        println!("{body}");
                    }
                }
            }
        }
        Ok(())
    }

    /// Displays the status of an issue.
    pub fn status_issue(&self, id: u64) -> Result<(), Box<dyn std::error::Error>> {
        let repo = self.repo();
        match repo.find_issue(id, None)? {
            None => {
                eprintln!("Issue #{id} not found.");
                process::exit(1);
            }
            Some(issue) => {
                println!(
                    "#{}: {} [{}]",
                    issue.id,
                    issue.meta.title,
                    issue.meta.state.as_str()
                );
            }
        }
        Ok(())
    }
}

fn run_inner(command: IssueCommand) -> Result<(), Box<dyn std::error::Error>> {
    let executor = Executor::from_env()?;

    match command {
        IssueCommand::New {
            title,
            body,
            label,
            assignee,
        } => {
            let body = if let Some(b) = body {
                b
            } else {
                use std::io::Read;
                let mut buf = String::new();
                std::io::stdin().read_to_string(&mut buf)?;
                buf
            };
            let id = executor.create_issue(&title, &body, Some(&label), Some(&assignee))?;
            eprintln!("Created issue #{id}: {title}");
        }

        IssueCommand::Edit {
            id,
            title,
            body,
            label,
            assignee,
            state,
        } => {
            let labels = if label.is_empty() { None } else { Some(label) };
            let assignees = if assignee.is_empty() {
                None
            } else {
                Some(assignee)
            };
            let issue_state = state.map(|s| match s {
                StateArg::Open => IssueState::Open,
                StateArg::Closed => IssueState::Closed,
            });
            executor.edit_issue(
                id,
                title.as_deref(),
                body.as_deref(),
                labels.as_deref(),
                assignees.as_deref(),
                issue_state,
            )?;
            eprintln!("Updated issue #{id}.");
        }

        IssueCommand::List { state } => {
            let issue_state = match state {
                StateArg::Open => IssueState::Open,
                StateArg::Closed => IssueState::Closed,
            };
            let repo = executor.repo();
            let issues = repo.list_issues_by_state(issue_state, None)?;
            if issues.is_empty() {
                println!("No {} issues.", issue_state.as_str());
            } else {
                for issue in &issues {
                    println!(
                        "#{:>4}  [{}]  {}",
                        issue.id,
                        issue.meta.state.as_str(),
                        issue.meta.title,
                    );
                }
            }
        }

        IssueCommand::Status { id } => {
            executor.status_issue(id)?;
        }

        IssueCommand::Show { id } => {
            executor.show_issue(id)?;
        }
    }

    Ok(())
}

/// Execute an `issue` subcommand.
pub fn run(command: IssueCommand) {
    if let Err(e) = run_inner(command) {
        eprintln!("Error: {e}");
        process::exit(1);
    }
}
