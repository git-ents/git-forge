//! `git-forge`: A Git subcommand for store, anchor, and query.

use std::{
    fmt::Write as _,
    io::{IsTerminal, Write as _},
};

use acdc_converters_core::{Converter as _, Diagnostics, Options as ConvertOptions, WarningSource};
use acdc_converters_terminal::Processor as TerminalProcessor;
use acdc_parser::{Options as ParseOptions, parse as parse_asciidoc};
use anyhow::{Context, Result, bail};
use chrono::{DateTime, Utc};
use clap::{Args, Parser, Subcommand};
use comfy_table::{
    Attribute, Cell, CellAlignment, ContentArrangement, Table, modifiers::UTF8_ROUND_CORNERS,
    presets::UTF8_FULL,
};
use gix_forge::{CommentEdit, Issue, QueryValue, Review, ReviewTarget};
use owo_colors::OwoColorize;

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
    Comment(CommentCommand),
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
    Edit(IssueEditArgs),
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
    #[arg(short = 'i', long = "interactive", conflicts_with_all = ["title", "body", "labels", "assignees", "reporters"])]
    interactive: bool,
    #[arg(long)]
    title: Option<String>,
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
    Edit(ReviewEditArgs),
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
enum CommentCommand {
    Edit(CommentEditArgs),
    Log { id: String },
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

#[derive(Args)]
struct IssueEditArgs {
    id: String,
    #[arg(long)]
    title: Option<String>,
    #[arg(long)]
    body: Option<String>,
    #[arg(long)]
    edit: String,
}

#[derive(Args)]
struct ReviewEditArgs {
    id: String,
    #[arg(long)]
    body: Option<String>,
    #[arg(long)]
    target: Option<String>,
    #[arg(long)]
    edit: String,
}

#[derive(Args)]
struct CommentEditArgs {
    id: String,
    #[arg(long)]
    edit: String,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let repo = gix::discover(".").context("not inside a git repository")?;

    match cli.command {
        Command::Issue(command) => run_issue(&repo, command)?,
        Command::Review(command) => run_review(&repo, command)?,
        Command::Comment(command) => run_comment(&repo, command)?,
        Command::Query(command) => run_query(&repo, command)?,
        Command::Install(args) => run_install(&repo, args)?,
    }

    Ok(())
}

fn run_issue(repo: &gix::Repository, command: IssueCommand) -> Result<()> {
    match command {
        IssueCommand::New(args) | IssueCommand::Put(args) => {
            let (title, body, labels, assignees, reporters) = if args.interactive {
                prompt_issue_fields()?
            } else {
                (
                    args.title.unwrap_or_default(),
                    args.body
                        .context("--body is required unless --interactive")?,
                    args.labels,
                    args.assignees,
                    args.reporters,
                )
            };
            let issue = Issue {
                id: origin_commit_short_id(repo)?,
                title,
                body,
                labels,
                assignees,
                reporters,
                edit: None,
            };
            println!("{}", issue.save_in_repo(repo)?);
        }
        IssueCommand::Edit(args) => {
            let id = resolve_issue_show_id(repo, &args.id)?
                .with_context(|| format!("no issue {}", args.id))?;
            let mut issue =
                Issue::load_from_repo(repo, &id)?.with_context(|| format!("no issue {id}"))?;
            if let Some(title) = args.title {
                issue.title = title;
            }
            if let Some(body) = args.body {
                issue.body = body;
            }
            issue.edit = Some(args.edit);
            println!("{}", issue.save_in_repo(repo)?);
        }
        IssueCommand::Show { id } => {
            let id = resolve_issue_show_id(repo, &id)?.with_context(|| format!("no issue {id}"))?;
            let issue =
                Issue::load_from_repo(repo, &id)?.with_context(|| format!("no issue {id}"))?;
            print_issue(&issue);
        }
        IssueCommand::List => print_issue_list(repo)?,
        IssueCommand::Log { id } => print_log("issue", &id, repo, Issue::history(repo, &id)?)?,
        IssueCommand::Rm { id } => {
            if !Issue::delete(repo, &id)? {
                bail!("no issue {id}");
            }
        }
    }
    Ok(())
}

fn run_comment(repo: &gix::Repository, command: CommentCommand) -> Result<()> {
    match command {
        CommentCommand::Edit(args) => {
            let edit = CommentEdit {
                id: args.id,
                edit: args.edit,
            };
            println!("{}", edit.save_in_repo(repo)?);
        }
        CommentCommand::Log { id } => {
            print_log("comment", &id, repo, CommentEdit::history(repo, &id)?)?;
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

fn prompt_issue_fields() -> Result<(String, String, Vec<String>, Vec<String>, Vec<String>)> {
    require_terminal_for_interactive()?;
    let title = prompt_line("title (optional)")?;
    let body = prompt_required("body")?;
    let labels = prompt_csv("labels (comma-separated, optional)")?;
    let assignees = prompt_csv("assignees (comma-separated, optional)")?;
    let reporters = prompt_csv("reporters (comma-separated, optional)")?;
    Ok((title, body, labels, assignees, reporters))
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
            let mut matches = Vec::new();
            for id in ids {
                if let Some(issue) = Issue::load_from_repo(repo, &id)?
                    && issue.assignees.iter().any(|assignee| assignee == &name)
                {
                    matches.push(issue.id);
                }
            }
            print_id_list(&format!("issues assigned to {name}"), &matches);
        }
        QueryCommand::Reviewer { name } => {
            let ids = Review::list(repo)?;
            let mut matches = Vec::new();
            for id in ids {
                if let Some(review) = Review::load_from_repo(repo, &id)?
                    && review.reviewers.iter().any(|reviewer| reviewer == &name)
                {
                    matches.push(review.id);
                }
            }
            print_id_list(&format!("reviews by reviewer {name}"), &matches);
        }
        QueryCommand::Requester { name } => {
            let ids = Review::list(repo)?;
            let mut matches = Vec::new();
            for id in ids {
                if let Some(review) = Review::load_from_repo(repo, &id)?
                    && review.requesters.iter().any(|requester| requester == &name)
                {
                    matches.push(review.id);
                }
            }
            print_id_list(&format!("reviews by requester {name}"), &matches);
        }
        QueryCommand::Keyword { value } => {
            let needle = value.to_ascii_lowercase();
            let mut matches = Vec::new();

            let issue_ids = Issue::list(repo)?;
            for id in issue_ids {
                if let Some(issue) = Issue::load_from_repo(repo, &id)?
                    && issue.body.to_ascii_lowercase().contains(&needle)
                {
                    matches.push(format!("issue {}", issue.id));
                }
            }

            let review_ids = Review::list(repo)?;
            for id in review_ids {
                if let Some(review) = Review::load_from_repo(repo, &id)?
                    && review.body.to_ascii_lowercase().contains(&needle)
                {
                    matches.push(format!("review {}", review.id));
                }
            }

            print_bulleted_section(&format!("matches for \"{value}\""), &matches);
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

            let mut matches = Vec::new();

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
                    matches.push(format!("issue {}", issue.id));
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
                    matches.push(format!("review {}", review.id));
                }
            }

            print_bulleted_section("query matches", &matches);
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
    println!("query results:");
    if rows.is_empty() {
        println!("  (none)");
        return;
    }
    for row in rows {
        let cols: Vec<String> = row.iter().map(ToString::to_string).collect();
        println!("  - {}", cols.join(" | "));
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
                edit: None,
            };
            println!("{}", review.save_in_repo(repo)?);
        }
        ReviewCommand::Edit(args) => {
            let id = resolve_review_show_id(repo, &args.id)?
                .with_context(|| format!("no review {}", args.id))?;
            let mut review =
                Review::load_from_repo(repo, &id)?.with_context(|| format!("no review {id}"))?;
            if let Some(body) = args.body {
                review.body = body;
            }
            if let Some(target) = args.target {
                review.target = parse_review_target(&target)?;
            }
            review.edit = Some(args.edit);
            println!("{}", review.save_in_repo(repo)?);
        }
        ReviewCommand::Show { id } => {
            let id =
                resolve_review_show_id(repo, &id)?.with_context(|| format!("no review {id}"))?;
            let review =
                Review::load_from_repo(repo, &id)?.with_context(|| format!("no review {id}"))?;
            print_review(&review);
        }
        ReviewCommand::List => {
            let ids = Review::list(repo)?;
            print_id_list("reviews", &ids);
        }
        ReviewCommand::Log { id } => print_log("review", &id, repo, Review::history(repo, &id)?)?,
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

fn print_id_list(title: &str, ids: &[String]) {
    print_bulleted_section(title, ids);
}

fn print_issue_list(repo: &gix::Repository) -> Result<()> {
    let ids = Issue::list(repo)?;
    if ids.is_empty() {
        println!("No issues");
        return Ok(());
    }

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            Cell::new("ID").add_attribute(Attribute::Bold),
            Cell::new("TITLE").add_attribute(Attribute::Bold),
            Cell::new("LABELS").add_attribute(Attribute::Bold),
            Cell::new("UPDATED").add_attribute(Attribute::Bold),
        ]);

    for id in ids {
        let Some(issue) = Issue::load_from_repo(repo, &id)? else {
            continue;
        };

        let title = if issue.title.trim().is_empty() {
            "(untitled)"
        } else {
            issue.title.as_str()
        };

        table.add_row(vec![
            Cell::new(format!("#{id}")).set_alignment(CellAlignment::Left),
            Cell::new(title).set_alignment(CellAlignment::Left),
            Cell::new(join_values_or_none(&issue.labels)).set_alignment(CellAlignment::Left),
            Cell::new(issue_updated_relative(repo, &id)?).set_alignment(CellAlignment::Left),
        ]);
    }

    println!("{table}");
    Ok(())
}

fn issue_updated_relative(repo: &gix::Repository, id: &str) -> Result<String> {
    let history = Issue::history(repo, id)?;
    let Some(oid) = history.first() else {
        return Ok("(unknown)".to_owned());
    };

    let commit = repo.find_commit(*oid)?;
    let when = commit.time()?.format(gix::date::time::format::ISO8601)?;
    Ok(relative_time_from_iso8601(&when).unwrap_or(when))
}

fn relative_time_from_iso8601(value: &str) -> Option<String> {
    let then = DateTime::parse_from_rfc3339(value)
        .ok()?
        .with_timezone(&Utc);
    let now = Utc::now();
    let delta = now.signed_duration_since(then);

    let (value, unit) = if delta.num_seconds() < 60 {
        return Some("just now".to_owned());
    } else if delta.num_minutes() < 60 {
        (delta.num_minutes(), "minute")
    } else if delta.num_hours() < 24 {
        (delta.num_hours(), "hour")
    } else if delta.num_days() < 30 {
        (delta.num_days(), "day")
    } else if delta.num_days() < 365 {
        (delta.num_days() / 30, "month")
    } else {
        (delta.num_days() / 365, "year")
    };

    let plural = if value == 1 { "" } else { "s" };
    Some(format!("about {value} {unit}{plural} ago"))
}

fn print_bulleted_section(title: &str, items: &[String]) {
    println!("{}:", color_field_name(title));
    if items.is_empty() {
        println!("  {}", color_empty_marker("(none)"));
        return;
    }
    for item in items {
        println!("  - {item}");
    }
}

fn print_log(
    kind: &str,
    id: &str,
    repo: &gix::Repository,
    commits: Vec<gix::ObjectId>,
) -> Result<()> {
    println!(
        "{} {} {}:",
        color_heading(kind),
        color_field_name("history"),
        color_id(id)
    );
    if commits.is_empty() {
        println!("  {}", color_empty_marker("(none)"));
        return Ok(());
    }
    for oid in commits {
        let commit = repo.find_commit(oid)?;
        let when = commit.time()?.format(gix::date::time::format::ISO8601)?;
        println!("  - {oid}  {when}");
    }
    Ok(())
}

struct Doc<'a> {
    kind: &'a str,
    id: &'a str,
    title: Option<&'a str>,
    fields: Vec<(&'a str, String)>,
    body: &'a str,
    edit: Option<&'a str>,
}

fn render_doc(doc: &Doc<'_>) -> String {
    let mut source = String::new();
    if let Some(title) = doc.title {
        let heading = if title.is_empty() {
            "(untitled)"
        } else {
            title
        };
        let _ = writeln!(&mut source, "= {heading}");
    } else {
        let _ = writeln!(&mut source, "= {} {}", doc.kind, doc.id);
    }
    source.push_str(":!sectnums:\n\n");
    source.push_str("[horizontal]\n");
    let _ = writeln!(&mut source, "kind:: {}", doc.kind);
    let _ = writeln!(&mut source, "id:: {}", doc.id);
    for (name, value) in &doc.fields {
        let _ = writeln!(&mut source, "{name}:: {value}");
    }
    let _ = writeln!(&mut source, "edit:: {}", doc.edit.unwrap_or("(none)"));
    source.push_str("\n== body\n\n");
    if doc.body.is_empty() {
        source.push_str("(none)\n");
    } else {
        source.push_str(doc.body);
        source.push('\n');
    }
    source
}

fn print_doc(doc: &Doc<'_>) {
    print_rendered(&render_asciidoc_terminal(&render_doc(doc)));
}

fn print_issue(issue: &Issue) {
    let title = if issue.title.trim().is_empty() {
        "(untitled)"
    } else {
        issue.title.as_str()
    };

    println!("{} {}", title.bold(), format!("#{}", issue.id).yellow());

    let mut meta = vec!["Open".green().bold().to_string()];
    if !issue.labels.is_empty() {
        meta.push(format!("labels: {}", issue.labels.join(", ")));
    }
    if !issue.assignees.is_empty() {
        meta.push(format!("assignees: {}", issue.assignees.join(", ")));
    }
    if !issue.reporters.is_empty() {
        meta.push(format!("reporters: {}", issue.reporters.join(", ")));
    }
    if let Some(edit) = &issue.edit {
        meta.push(format!("edit: {edit}"));
    }

    let separator = format!(" {} ", "•".dimmed());
    println!("{}", meta.join(&separator));
    println!();

    if issue.body.trim().is_empty() {
        println!("{}", color_empty_marker("(none)"));
        return;
    }

    print_rendered(&render_asciidoc_terminal(&issue.body));
}

fn print_review(review: &Review) {
    let doc = Doc {
        kind: "review",
        id: &review.id,
        title: None,
        fields: vec![
            ("reviewers", join_values_or_none(&review.reviewers)),
            ("requesters", join_values_or_none(&review.requesters)),
            ("target", format_target(&review.target)),
        ],
        body: &review.body,
        edit: review.edit.as_deref(),
    };
    print_doc(&doc);
}

fn render_asciidoc_terminal(value: &str) -> String {
    let parsed = match parse_asciidoc(value, &ParseOptions::default()) {
        Ok(parsed) => parsed,
        Err(_) => return value.to_owned(),
    };

    let document = parsed.document();
    let processor = TerminalProcessor::new(ConvertOptions::default(), document.attributes.clone());
    let source = WarningSource::new("git-forge");
    let mut warnings = Vec::new();
    let mut diagnostics = Diagnostics::new(&source, &mut warnings);
    let mut output = Vec::new();

    if processor
        .write_to(document, &mut output, None, None, &mut diagnostics)
        .is_err()
    {
        return value.to_owned();
    }

    String::from_utf8_lossy(&output).to_string()
}

fn print_rendered(rendered: &str) {
    if rendered.ends_with('\n') {
        print!("{rendered}");
    } else {
        println!("{rendered}");
    }
}

fn join_values_or_none(values: &[String]) -> String {
    if values.is_empty() {
        "(none)".to_owned()
    } else {
        values.join(", ")
    }
}

fn color_heading(value: &str) -> String {
    colorize(value, "1;36")
}

fn color_field_name(value: &str) -> String {
    colorize(value, "1;34")
}

fn color_id(value: &str) -> String {
    colorize(value, "33")
}

fn color_empty_marker(value: &str) -> String {
    colorize(value, "2")
}

fn colorize(value: &str, code: &str) -> String {
    if color_output_enabled() {
        format!("\u{1b}[{code}m{value}\u{1b}[0m")
    } else {
        value.to_owned()
    }
}

fn color_output_enabled() -> bool {
    std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none()
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
