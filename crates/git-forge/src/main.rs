//! `git-forge`: A Git subcommand for store, anchor, and query.

use std::io::{IsTerminal, Write as _};

use anyhow::{Context, Result, bail};
use clap::{Args, Parser, Subcommand};
use gix_forge::{Issue, QueryValue, Review, ReviewTarget};

#[derive(Parser)]
#[command(name = "git-forge", about = "Forge software on Git", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    #[command(subcommand)]
    Issue(IssueCommand),
    #[command(subcommand)]
    Review(ReviewCommand),
    #[command(subcommand)]
    Query(QueryCommand),
    Install(InstallArgs),
}

#[derive(Args)]
struct InstallArgs {
    #[arg(short = 'i', long = "interactive")]
    interactive: bool,
}

#[derive(Subcommand)]
enum IssueCommand {
    New(IssueNewArgs),
    #[command(hide = true)]
    Put(IssueNewArgs),
    Show {
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
struct IssueNewArgs {
    #[arg(short = 'i', long = "interactive", conflicts_with_all = ["body", "labels", "assignees", "reporters"])]
    interactive: bool,
    #[arg(long, required_unless_present = "interactive")]
    body: Option<String>,
    #[arg(long = "label")]
    labels: Vec<String>,
    #[arg(long = "assignee")]
    assignees: Vec<String>,
    #[arg(long = "reporter")]
    reporters: Vec<String>,
}

#[derive(Subcommand)]
enum ReviewCommand {
    New(ReviewNewArgs),
    #[command(hide = true)]
    Put(ReviewNewArgs),
    Show {
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

#[derive(Subcommand)]
enum QueryCommand {
    Run {
        #[arg(conflicts_with = "goal")]
        predicate: Option<String>,
        #[arg(long = "bind", value_name = "POSITION=VALUE")]
        bind: Vec<String>,
        #[arg(long)]
        goal: Option<String>,
        #[arg(long, value_delimiter = ',')]
        select: Vec<String>,
    },
    Assignee {
        name: String,
    },
    Reviewer {
        name: String,
    },
    Requester {
        name: String,
    },
    #[command(visible_alias = "title")]
    Keyword {
        value: String,
    },
    Find {
        #[arg(long)]
        assignee: Option<String>,
        #[arg(long)]
        reviewer: Option<String>,
        #[arg(long)]
        requester: Option<String>,
        #[arg(long)]
        keyword: Option<String>,
        #[arg(long, conflicts_with = "keyword")]
        title: Option<String>,
    },
}

#[derive(Args)]
struct ReviewNewArgs {
    #[arg(short = 'i', long = "interactive", conflicts_with_all = ["body", "reviewers", "requesters", "target"])]
    interactive: bool,
    #[arg(long, required_unless_present = "interactive")]
    body: Option<String>,
    #[arg(long = "reviewer")]
    reviewers: Vec<String>,
    #[arg(long = "requester")]
    requesters: Vec<String>,
    #[arg(long, value_name = "TARGET", required_unless_present = "interactive")]
    target: Option<String>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let repo = gix::discover(".").context("not inside a git repository")?;

    match cli.command {
        Command::Issue(command) => run_issue(&repo, command)?,
        Command::Review(command) => run_review(&repo, command)?,
        Command::Query(command) => run_query(&repo, command)?,
        Command::Install(args) => run_install(&repo, args)?,
    }

    Ok(())
}

fn run_issue(repo: &gix::Repository, command: IssueCommand) -> Result<()> {
    match command {
        IssueCommand::New(args) | IssueCommand::Put(args) => {
            let (body, labels, assignees, reporters) = if args.interactive {
                prompt_issue_fields()?
            } else {
                (
                    args.body
                        .context("--body is required unless --interactive")?,
                    args.labels,
                    args.assignees,
                    args.reporters,
                )
            };
            let issue = Issue {
                id: origin_commit_short_id(repo)?,
                body,
                labels,
                assignees,
                reporters,
            };
            println!("{}", issue.save_in_repo(repo)?);
        }
        IssueCommand::Show { id } => {
            let id = resolve_issue_show_id(repo, &id)?.with_context(|| format!("no issue {id}"))?;
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

fn run_install(repo: &gix::Repository, args: InstallArgs) -> Result<()> {
    let mut installed = false;

    if should_install(args.interactive, "issue schema")? {
        println!("{} {}", Issue::KIND, gix_forge::ensure_issue_schema(repo)?);
        installed = true;
    }

    if should_install(args.interactive, "review schema")? {
        println!(
            "{} {}",
            Review::KIND,
            gix_forge::ensure_review_schema(repo)?
        );
        installed = true;
    }

    if should_install(args.interactive, "query rules")? {
        gix_forge::install_builtin_query_rules(repo)?;
        println!("query rules review");
        installed = true;
    }

    if !installed {
        bail!("no schemas selected for installation");
    }

    Ok(())
}

fn should_install(interactive: bool, prompt: &str) -> Result<bool> {
    if !interactive {
        return Ok(true);
    }
    if !std::io::stdin().is_terminal() {
        bail!("--interactive requires a terminal");
    }

    loop {
        eprint!("install {prompt}? [Y/n]: ");
        std::io::stderr().flush()?;

        let mut input = String::new();
        let read = std::io::stdin().read_line(&mut input)?;
        if read == 0 {
            bail!("unexpected end of input");
        }

        match input.trim().to_ascii_lowercase().as_str() {
            "" | "y" | "yes" => return Ok(true),
            "n" | "no" => return Ok(false),
            _ => eprintln!("please answer y or n"),
        }
    }
}

fn prompt_issue_fields() -> Result<(String, Vec<String>, Vec<String>, Vec<String>)> {
    require_terminal_for_interactive()?;
    let body = prompt_required("body")?;
    let labels = prompt_csv("labels (comma-separated, optional)")?;
    let assignees = prompt_csv("assignees (comma-separated, optional)")?;
    let reporters = prompt_csv("reporters (comma-separated, optional)")?;
    Ok((body, labels, assignees, reporters))
}

fn prompt_review_fields() -> Result<(String, Vec<String>, Vec<String>, String)> {
    require_terminal_for_interactive()?;
    let body = prompt_required("body")?;
    let reviewers = prompt_csv("reviewers (comma-separated, optional)")?;
    let requesters = prompt_csv("requesters (comma-separated, optional)")?;
    let target = prompt_required("target")?;
    Ok((body, reviewers, requesters, target))
}

fn require_terminal_for_interactive() -> Result<()> {
    if std::io::stdin().is_terminal() {
        Ok(())
    } else {
        bail!("--interactive requires a terminal")
    }
}

fn prompt_required(field: &str) -> Result<String> {
    loop {
        let input = prompt_line(field)?;
        if !input.trim().is_empty() {
            return Ok(input);
        }
        eprintln!("{field} is required");
    }
}

fn prompt_csv(field: &str) -> Result<Vec<String>> {
    let input = prompt_line(field)?;
    Ok(input
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
        .collect())
}

fn prompt_line(field: &str) -> Result<String> {
    eprint!("{field}: ");
    std::io::stderr().flush()?;

    let mut input = String::new();
    let read = std::io::stdin().read_line(&mut input)?;
    if read == 0 {
        bail!("unexpected end of input");
    }

    Ok(input.trim().to_owned())
}

fn origin_commit_short_id(repo: &gix::Repository) -> Result<String> {
    let id = repo
        .head_id()
        .context("cannot create issue/review without a checked-out commit")?;
    let full = id.to_string();
    Ok(full.chars().take(8).collect())
}

fn run_query(repo: &gix::Repository, command: QueryCommand) -> Result<()> {
    match command {
        QueryCommand::Run {
            predicate,
            bind,
            goal,
            select,
        } => match (predicate, goal) {
            (Some(predicate), None) => {
                let bound: Vec<(usize, QueryValue)> = bind
                    .iter()
                    .map(|item| parse_bind(item))
                    .collect::<Result<_>>()?;
                let rows = gix_forge::query_predicate(repo, &predicate, &bound)?;
                print_rows(&rows);
            }
            (None, Some(goal)) => {
                if select.is_empty() {
                    bail!("--goal requires --select");
                }
                let select: Vec<&str> = select.iter().map(String::as_str).collect();
                let rows = gix_forge::query_goal(repo, &goal, &select)?;
                print_rows(&rows);
            }
            (None, None) => bail!("run requires either a predicate or --goal"),
            (Some(_), Some(_)) => unreachable!("clap rejects predicate with --goal"),
        },
        QueryCommand::Assignee { name } => {
            let ids = Issue::list(repo)?;
            for id in ids {
                if let Some(issue) = Issue::load_from_repo(repo, &id)?
                    && issue.assignees.iter().any(|assignee| assignee == &name)
                {
                    println!("{}", issue.id);
                }
            }
        }
        QueryCommand::Reviewer { name } => {
            let ids = Review::list(repo)?;
            for id in ids {
                if let Some(review) = Review::load_from_repo(repo, &id)?
                    && review.reviewers.iter().any(|reviewer| reviewer == &name)
                {
                    println!("{}", review.id);
                }
            }
        }
        QueryCommand::Requester { name } => {
            let ids = Review::list(repo)?;
            for id in ids {
                if let Some(review) = Review::load_from_repo(repo, &id)?
                    && review.requesters.iter().any(|requester| requester == &name)
                {
                    println!("{}", review.id);
                }
            }
        }
        QueryCommand::Keyword { value } => {
            let needle = value.to_ascii_lowercase();

            let issue_ids = Issue::list(repo)?;
            for id in issue_ids {
                if let Some(issue) = Issue::load_from_repo(repo, &id)?
                    && issue.body.to_ascii_lowercase().contains(&needle)
                {
                    println!("issue:{}", issue.id);
                }
            }

            let review_ids = Review::list(repo)?;
            for id in review_ids {
                if let Some(review) = Review::load_from_repo(repo, &id)?
                    && review.body.to_ascii_lowercase().contains(&needle)
                {
                    println!("review:{}", review.id);
                }
            }
        }
        QueryCommand::Find {
            assignee,
            reviewer,
            requester,
            keyword,
            title,
        } => {
            let needle = keyword.or(title).map(|value| value.to_ascii_lowercase());
            if assignee.is_none() && reviewer.is_none() && requester.is_none() && needle.is_none() {
                bail!(
                    "query find requires at least one filter: --assignee, --reviewer, --requester, --keyword, or --title"
                );
            }

            let issue_ids = Issue::list(repo)?;
            for id in issue_ids {
                if let Some(issue) = Issue::load_from_repo(repo, &id)? {
                    if let Some(name) = &assignee
                        && !issue.assignees.iter().any(|a| a == name)
                    {
                        continue;
                    }
                    if reviewer.is_some() || requester.is_some() {
                        continue;
                    }
                    if let Some(needle) = &needle
                        && !issue.body.to_ascii_lowercase().contains(needle)
                    {
                        continue;
                    }
                    println!("issue:{}", issue.id);
                }
            }

            let review_ids = Review::list(repo)?;
            for id in review_ids {
                if let Some(review) = Review::load_from_repo(repo, &id)? {
                    if assignee.is_some() {
                        continue;
                    }
                    if let Some(name) = &reviewer
                        && !review.reviewers.iter().any(|r| r == name)
                    {
                        continue;
                    }
                    if let Some(name) = &requester
                        && !review.requesters.iter().any(|r| r == name)
                    {
                        continue;
                    }
                    if let Some(needle) = &needle
                        && !review.body.to_ascii_lowercase().contains(needle)
                    {
                        continue;
                    }
                    println!("review:{}", review.id);
                }
            }
        }
    }
    Ok(())
}

fn parse_bind(arg: &str) -> Result<(usize, QueryValue)> {
    let (position, value) = arg
        .split_once('=')
        .with_context(|| format!("`--bind {arg}` is not `<position>=<value>`"))?;
    let position: usize = position
        .parse()
        .with_context(|| format!("`--bind {arg}`: `{position}` is not a position"))?;

    let value = match value.parse::<i64>() {
        Ok(n) => QueryValue::Int(n),
        Err(_) => QueryValue::Sym(value.into()),
    };

    Ok((position, value))
}

fn print_rows(rows: &[Vec<QueryValue>]) {
    for row in rows {
        let cols: Vec<String> = row.iter().map(ToString::to_string).collect();
        println!("{}", cols.join("\t"));
    }
}

fn run_review(repo: &gix::Repository, command: ReviewCommand) -> Result<()> {
    match command {
        ReviewCommand::New(args) | ReviewCommand::Put(args) => {
            let (body, reviewers, requesters, target) = if args.interactive {
                prompt_review_fields()?
            } else {
                (
                    args.body
                        .context("--body is required unless --interactive")?,
                    args.reviewers,
                    args.requesters,
                    args.target
                        .context("--target is required unless --interactive")?,
                )
            };
            let review = Review {
                id: origin_commit_short_id(repo)?,
                body,
                reviewers,
                requesters,
                target: parse_review_target(&target)?,
            };
            println!("{}", review.save_in_repo(repo)?);
        }
        ReviewCommand::Show { id } => {
            let id =
                resolve_review_show_id(repo, &id)?.with_context(|| format!("no review {id}"))?;
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

fn resolve_issue_show_id(repo: &gix::Repository, id: &str) -> Result<Option<String>> {
    let ids = Issue::list(repo)?;
    resolve_show_id("issue", id, &ids)
}

fn resolve_review_show_id(repo: &gix::Repository, id: &str) -> Result<Option<String>> {
    let ids = Review::list(repo)?;
    resolve_show_id("review", id, &ids)
}

fn resolve_show_id(kind: &str, input: &str, ids: &[String]) -> Result<Option<String>> {
    if ids.iter().any(|id| id == input) {
        return Ok(Some(input.to_owned()));
    }

    let matches: Vec<&String> = ids.iter().filter(|id| id.starts_with(input)).collect();
    match matches.as_slice() {
        [] => Ok(None),
        [only] => Ok(Some((**only).clone())),
        many => {
            let options = many
                .iter()
                .map(|id| format!("  {}", emphasize_min_unique_prefix(id, ids)))
                .collect::<Vec<_>>()
                .join("\n");
            bail!("ambiguous {kind} id {input}; matches:\n{options}")
        }
    }
}

fn emphasize_min_unique_prefix(id: &str, ids: &[String]) -> String {
    let min_len = min_unique_prefix_len(id, ids);
    let (prefix, suffix) = id.split_at(min_len);
    format!("**{prefix}**{suffix}")
}

fn min_unique_prefix_len(id: &str, ids: &[String]) -> usize {
    for len in 1..=id.len() {
        let prefix = &id[..len];
        if ids
            .iter()
            .filter(|candidate| candidate.starts_with(prefix))
            .count()
            == 1
        {
            return len;
        }
    }
    id.len()
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
