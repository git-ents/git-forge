//! `git-forge`: A Git subcommand for store, anchor, and query.

use anyhow::{Context, Result, bail};
use clap::{Args, Parser, Subcommand};
use gix_forge::{Issue, Review, ReviewTarget};

#[derive(Parser)]
#[command(name = "git-forge", about = "Forge software on Git", version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    #[command(subcommand)]
    Issue(IssueCommand),
    #[command(subcommand)]
    Review(ReviewCommand),
}

#[derive(Subcommand)]
enum IssueCommand {
    Put(IssuePutArgs),
    Get {
        id: String,
    },
    #[command(visible_alias = "ls")]
    List,
    Log {
        id: String,
    },
    Rm {
        id: String,
    },
}

#[derive(Args)]
struct IssuePutArgs {
    id: String,
    #[arg(long)]
    body: String,
    #[arg(long = "label")]
    labels: Vec<String>,
    #[arg(long = "assignee")]
    assignees: Vec<String>,
    #[arg(long = "reporter")]
    reporters: Vec<String>,
}

#[derive(Subcommand)]
enum ReviewCommand {
    Put(ReviewPutArgs),
    Get {
        id: String,
    },
    #[command(visible_alias = "ls")]
    List,
    Log {
        id: String,
    },
    Rm {
        id: String,
    },
}

#[derive(Args)]
struct ReviewPutArgs {
    id: String,
    #[arg(long)]
    body: String,
    #[arg(long = "reviewer")]
    reviewers: Vec<String>,
    #[arg(long = "requester")]
    requesters: Vec<String>,
    #[arg(long, value_name = "TARGET")]
    target: String,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let repo = gix::discover(".").context("not inside a git repository")?;

    match cli.command {
        None => {
            println!("issue");
            println!("review");
        }
        Some(Command::Issue(command)) => run_issue(&repo, command)?,
        Some(Command::Review(command)) => run_review(&repo, command)?,
    }

    Ok(())
}

fn run_issue(repo: &gix::Repository, command: IssueCommand) -> Result<()> {
    match command {
        IssueCommand::Put(args) => {
            let issue = Issue {
                id: args.id,
                body: args.body,
                labels: args.labels,
                assignees: args.assignees,
                reporters: args.reporters,
            };
            println!("{}", issue.save_in_repo(repo)?);
        }
        IssueCommand::Get { id } => {
            let issue =
                Issue::load_from_repo(repo, &id)?.with_context(|| format!("no issue {id}"))?;
            print_issue(&issue);
        }
        IssueCommand::List => print_lines(Issue::list(repo)?),
        IssueCommand::Log { id } => print_log(repo, Issue::history(repo, &id)?)?,
        IssueCommand::Rm { id } => {
            if !Issue::delete(repo, &id)? {
                bail!("no issue {id}");
            }
        }
    }
    Ok(())
}

fn run_review(repo: &gix::Repository, command: ReviewCommand) -> Result<()> {
    match command {
        ReviewCommand::Put(args) => {
            let review = Review {
                id: args.id,
                body: args.body,
                reviewers: args.reviewers,
                requesters: args.requesters,
                target: parse_review_target(&args.target)?,
            };
            println!("{}", review.save_in_repo(repo)?);
        }
        ReviewCommand::Get { id } => {
            let review =
                Review::load_from_repo(repo, &id)?.with_context(|| format!("no review {id}"))?;
            print_review(&review);
        }
        ReviewCommand::List => print_lines(Review::list(repo)?),
        ReviewCommand::Log { id } => print_log(repo, Review::history(repo, &id)?)?,
        ReviewCommand::Rm { id } => {
            if !Review::delete(repo, &id)? {
                bail!("no review {id}");
            }
        }
    }
    Ok(())
}

fn parse_review_target(target: &str) -> Result<ReviewTarget> {
    if let Some(rest) = target.strip_prefix("commit:") {
        return Ok(ReviewTarget::Commit {
            oid: rest.to_owned(),
        });
    }
    if let Some(rest) = target.strip_prefix("tree:") {
        return Ok(ReviewTarget::Tree {
            oid: rest.to_owned(),
        });
    }
    if let Some(rest) = target.strip_prefix("blob:") {
        let mut parts = rest.splitn(2, ':');
        let path = parts.next().unwrap_or_default();
        let oid = parts.next().unwrap_or_default();
        if path.is_empty() || oid.is_empty() {
            bail!("blob target must be blob:<path>:<oid>");
        }
        return Ok(ReviewTarget::Blob {
            path: path.to_owned(),
            oid: oid.to_owned(),
        });
    }
    if let Some(rest) = target.strip_prefix("base-tip-tree:") {
        let mut parts = rest.splitn(2, ':');
        let base = parts.next().unwrap_or_default();
        let tip = parts.next().unwrap_or_default();
        if base.is_empty() || tip.is_empty() {
            bail!("base-tip-tree target must be base-tip-tree:<base>:<tip>");
        }
        return Ok(ReviewTarget::BaseTipTreePair {
            base: base.to_owned(),
            tip: tip.to_owned(),
        });
    }
    if let Some(rest) = target.strip_prefix("base-tip-commit:") {
        let mut parts = rest.splitn(2, ':');
        let base = parts.next().unwrap_or_default();
        let tip = parts.next().unwrap_or_default();
        if base.is_empty() || tip.is_empty() {
            bail!("base-tip-commit target must be base-tip-commit:<base>:<tip>");
        }
        return Ok(ReviewTarget::BaseTipCommitPair {
            base: base.to_owned(),
            tip: tip.to_owned(),
        });
    }
    if let Some(rest) = target.strip_prefix("commit-range:") {
        let mut parts = rest.splitn(2, ':');
        let start = parts.next().unwrap_or_default();
        let end = parts.next().unwrap_or_default();
        if start.is_empty() || end.is_empty() {
            bail!("commit-range target must be commit-range:<start>:<end>");
        }
        return Ok(ReviewTarget::CommitRange {
            start: start.to_owned(),
            end: end.to_owned(),
        });
    }
    Ok(ReviewTarget::Commit {
        oid: target.to_owned(),
    })
}

fn print_lines(items: Vec<String>) {
    for item in items {
        println!("{item}");
    }
}

fn print_log(repo: &gix::Repository, commits: Vec<gix::ObjectId>) -> Result<()> {
    for id in commits {
        let commit = repo.find_commit(id)?;
        let when = commit.time()?.format(gix::date::time::format::ISO8601)?;
        println!("{id} {when}");
    }
    Ok(())
}

fn print_issue(issue: &Issue) {
    println!("id: {}", issue.id);
    println!("body: {}", issue.body);
    println!("labels: {}", issue.labels.join(","));
    println!("assignees: {}", issue.assignees.join(","));
    println!("reporters: {}", issue.reporters.join(","));
}

fn print_review(review: &Review) {
    println!("id: {}", review.id);
    println!("body: {}", review.body);
    println!("reviewers: {}", review.reviewers.join(","));
    println!("requesters: {}", review.requesters.join(","));
    println!("target: {}", format_target(&review.target));
}

fn format_target(target: &ReviewTarget) -> String {
    match target {
        ReviewTarget::Blob { path, oid } => format!("blob:{path}:{oid}"),
        ReviewTarget::Tree { oid } => format!("tree:{oid}"),
        ReviewTarget::Commit { oid } => format!("commit:{oid}"),
        ReviewTarget::BaseTipTreePair { base, tip } => {
            format!("base-tip-tree:{base}:{tip}")
        }
        ReviewTarget::BaseTipCommitPair { base, tip } => {
            format!("base-tip-commit:{base}:{tip}")
        }
        ReviewTarget::CommitRange { start, end } => format!("commit-range:{start}:{end}"),
    }
}
