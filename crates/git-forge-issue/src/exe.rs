//! Execution logic for `git forge issue`.

use std::process;

use git2::Repository;

use crate::cli::{IssueCommand, StateArg};
use crate::{IssueState, Issues};

/// Resolve the editor to use, matching Git's own precedence:
/// `GIT_EDITOR` → `core.editor` (git config) → `VISUAL` → `EDITOR` → `"vi"`.
fn resolve_editor(repo: &git2::Repository) -> String {
    if let Ok(val) = std::env::var("GIT_EDITOR")
        && !val.is_empty() {
            return val;
        }
    if let Ok(cfg) = repo.config()
        && let Ok(val) = cfg.get_string("core.editor")
            && !val.is_empty() {
                return val;
            }
    for var in &["VISUAL", "EDITOR"] {
        if let Ok(val) = std::env::var(var)
            && !val.is_empty() {
                return val;
            }
    }
    "vi".to_string()
}

/// Parse issue template: first line is the title, optional blank line, rest is body.
/// Returns (title, body).
fn parse_issue_template(content: &str) -> Result<(String, String), Box<dyn std::error::Error>> {
    let mut lines = content.splitn(2, '\n');
    let title = lines.next().unwrap_or("").trim_end().to_string();
    let body = match lines.next() {
        None | Some("") => String::new(),
        Some(rest) => rest.strip_prefix('\n').unwrap_or(rest).trim_end().to_string(),
    };
    Ok((title, body))
}

fn read_issue_from_editor(
    repo: &git2::Repository,
) -> Result<(String, String), Box<dyn std::error::Error>> {
    use std::fs;
    use std::io::Write;
    use std::process::Command;

    let editor = resolve_editor(repo);
    let edit_path = repo.path().join("ISSUE_EDITMSG");
    let template = "\n\n";
    {
        let mut f = fs::File::create(&edit_path)?;
        f.write_all(template.as_bytes())?;
    }

    let status = Command::new(&editor).arg(&edit_path).status()?;
    if !status.success() {
        return Err("Editor exited with error".into());
    }

    let content = fs::read_to_string(&edit_path)?;
    let (title, body) = parse_issue_template(&content)?;
    if title.trim().is_empty() {
        return Err("Title cannot be empty".into());
    }
    Ok((title, body))
}

/// A wrapper type which manipulates issues for the provided repository.
struct Executor(git2::Repository);

impl Executor {
    /// Constructs an `Executor` from [`Repository::open_from_env()`].
    pub fn from_env() -> Result<Self, git2::Error> {
        let repo = Repository::open_from_env()?;
        Ok(Self(repo))
    }

    /// Return a reference the underlying [`git2::Repository`].
    pub fn repo(&self) -> &git2::Repository {
        &self.0
    }

    /// Updates an existing issue's text fields.
    pub fn edit_issue(
        &self,
        id: u64,
        title: Option<&str>,
        body: Option<&str>,
        state: Option<IssueState>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let repo = self.repo();
        repo.update_issue(id, title, body, None, None, state, None)?;
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

    /// Displays a one-line summary of an issue.
    pub fn show_issue_oneline(&self, id: u64) -> Result<(), Box<dyn std::error::Error>> {
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
        IssueCommand::New { title, body } => {
            use std::io::IsTerminal;

            let (title, body) =
                if title.is_none() && std::io::stdin().is_terminal() {
                    read_issue_from_editor(executor.repo())?
                } else {
                    let t = title.ok_or("Title is required")?;
                    let b = if let Some(b) = body {
                        b
                    } else {
                        use std::io::Read;
                        let mut buf = String::new();
                        std::io::stdin().read_to_string(&mut buf)?;
                        buf
                    };
                    (t, b)
                };

            let repo = executor.repo();
            let id = repo.create_issue(&title, &body, &[], &[], None)?;
            eprintln!("Created issue #{id}: {title}");
        }

        IssueCommand::Edit { id, title, body } => {
            let has_fields = title.is_some() || body.is_some();

            let repo = executor.repo();

            if has_fields {
                executor.edit_issue(id, title.as_deref(), body.as_deref(), None)?;
                eprintln!("Updated issue #{id}.");
            } else {
                use std::fs;
                use std::io::Write;
                use std::process::Command;

                let editor = resolve_editor(repo);

                let issue = repo
                    .find_issue(id, None)?
                    .ok_or(format!("Issue #{id} not found"))?;

                let edit_path = repo.path().join("ISSUE_EDITMSG");
                let template = format!("{}\n\n{}", issue.meta.title, issue.body);
                {
                    let mut f = fs::File::create(&edit_path)?;
                    f.write_all(template.as_bytes())?;
                }

                let status = Command::new(&editor).arg(&edit_path).status()?;
                if !status.success() {
                    return Err("Editor exited with error".into());
                }

                let content = fs::read_to_string(&edit_path)?;
                let (title, body) = parse_issue_template(&content)?;
                if title.trim().is_empty() {
                    return Err("Title cannot be empty".into());
                }

                repo.update_issue(id, Some(&title), Some(&body), None, None, None, None)?;
                eprintln!("Updated issue #{id}.");
            }
        }

        IssueCommand::List { state, labels, assignees } => {
            let repo = executor.repo();
            let issues = match state {
                StateArg::Open => repo.list_issues_by_state(IssueState::Open, None)?,
                StateArg::Closed => repo.list_issues_by_state(IssueState::Closed, None)?,
                StateArg::All => repo.list_issues(None)?,
            };
            let issues: Vec<_> = issues
                .into_iter()
                .filter(|i| labels.is_empty() || labels.iter().any(|l| i.meta.labels.contains(l)))
                .filter(|_| assignees.is_empty()) // assignees not yet surfaced on IssueMeta
                .collect();
            let empty_msg = match state {
                StateArg::Open => "No open issues.",
                StateArg::Closed => "No closed issues.",
                StateArg::All => "No issues.",
            };
            if issues.is_empty() {
                println!("{empty_msg}");
            } else {
                for issue in &issues {
                    println!(
                        "#{} [{}] {}",
                        issue.id,
                        issue.meta.state.as_str(),
                        issue.meta.title,
                    );
                }
            }
        }

        IssueCommand::Show { id, oneline } => {
            if oneline {
                executor.show_issue_oneline(id)?;
            } else {
                executor.show_issue(id)?;
            }
        }

        IssueCommand::Close { id } => {
            executor.edit_issue(id, None, None, Some(IssueState::Closed))?;
            eprintln!("Closed issue #{id}.");
        }

        IssueCommand::Reopen { id } => {
            executor.edit_issue(id, None, None, Some(IssueState::Open))?;
            eprintln!("Reopened issue #{id}.");
        }

        IssueCommand::Label { id, add, remove } => {
            let repo = executor.repo();
            let issue = repo
                .find_issue(id, None)?
                .ok_or(format!("Issue #{id} not found"))?;
            let mut labels = issue.meta.labels.clone();
            for l in &add {
                if !labels.contains(l) {
                    labels.push(l.clone());
                }
            }
            labels.retain(|l| !remove.contains(l));
            repo.update_issue(id, None, None, Some(&labels), None, None, None)?;
            eprintln!("Updated labels on issue #{id}.");
        }

        IssueCommand::Assign { id, add, remove } => {
            let repo = executor.repo();
            // assignees are not yet surfaced on IssueMeta; pass incremental sets directly
            let _ = remove;
            repo.update_issue(id, None, None, None, Some(&add), None, None)?;
            eprintln!("Updated assignees on issue #{id}.");
        }

        IssueCommand::Comment { id, body } => {
            use std::io::IsTerminal;

            let repo = executor.repo();
            let body = if let Some(b) = body {
                b
            } else if std::io::stdin().is_terminal() {
                use std::fs;
                use std::io::Write;
                use std::process::Command;

                let editor = resolve_editor(repo);
                let edit_path = repo.path().join("ISSUE_EDITMSG");
                {
                    let mut f = fs::File::create(&edit_path)?;
                    f.write_all(b"")?;
                }
                let status = Command::new(&editor).arg(&edit_path).status()?;
                if !status.success() {
                    return Err("Editor exited with error".into());
                }
                fs::read_to_string(&edit_path)?
            } else {
                use std::io::Read;
                let mut buf = String::new();
                std::io::stdin().read_to_string(&mut buf)?;
                buf
            };

            let cfg = repo.config()?;
            let author = cfg
                .get_string("user.email")
                .unwrap_or_else(|_| "unknown".to_string());
            repo.add_issue_comment(id, &author, &body, None)?;
            eprintln!("Added comment to issue #{id}.");
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
