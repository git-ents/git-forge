//! `git-forge`: A Git subcommand for store, anchor, and query.

use std::io::IsTerminal;

use acdc_converters_core::{Converter as _, Diagnostics, Options as ConvertOptions, WarningSource};
use acdc_converters_terminal::Processor as TerminalProcessor;
use acdc_parser::{Options as ParseOptions, parse as parse_asciidoc};
use anyhow::{Context, Result, bail};
use chrono::{DateTime, Utc};
use clap::{Args, Parser, Subcommand, ValueEnum};
use comfy_table::{Attribute, Cell, CellAlignment, ContentArrangement, Table, presets::NOTHING};
use dialoguer::{
    Confirm, Editor as InteractiveEditor, Input, MultiSelect, Select, theme::ColorfulTheme,
};
use gix_forge::{Comment, Entity, EntityOps, Issue, QueryValue, Review, ReviewTarget, Status};
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
    #[command(alias = "new")]
    #[command(alias = "put")]
    Add(IssueNewArgs),
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
    Search(IssueSearchArgs),
    Query(QueryArgs),
}

#[derive(Args)]
struct IssueSearchArgs {
    #[arg(long)]
    assignee: Option<String>,
    #[arg(long)]
    keyword: Option<String>,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum StatusArg {
    Open,
    Closed,
}

impl StatusArg {
    const fn as_status(self) -> Status {
        match self {
            StatusArg::Open => Status::Open,
            StatusArg::Closed => Status::Closed,
        }
    }
}

#[derive(Args)]
struct IssueNewArgs {
    #[arg(short = 'i', long = "interactive", conflicts_with_all = ["title", "body", "labels", "assignees", "reporters", "status"])]
    interactive: bool,
    #[arg(long)]
    title: Option<String>,
    #[arg(long)]
    body: Option<String>,
    #[arg(long, value_enum)]
    status: Option<StatusArg>,
    #[arg(long = "label")]
    labels: Vec<String>,
    #[arg(long = "assignee")]
    assignees: Vec<String>,
    #[arg(long = "reporter")]
    reporters: Vec<String>,
}

#[derive(Subcommand)]
enum ReviewCommand {
    #[command(alias = "new")]
    #[command(alias = "put")]
    Add(ReviewNewArgs),
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
    Search(ReviewSearchArgs),
    Query(QueryArgs),
}

#[derive(Args)]
struct ReviewSearchArgs {
    #[arg(long)]
    reviewer: Option<String>,
    #[arg(long)]
    requester: Option<String>,
    #[arg(long)]
    keyword: Option<String>,
}

#[derive(Subcommand)]
enum CommentCommand {
    #[command(alias = "new")]
    Add(CommentAddArgs),
    Edit(CommentEditArgs),
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
    Search(CommentSearchArgs),
    Query(QueryArgs),
}

#[derive(Args)]
struct CommentSearchArgs {
    #[arg(long)]
    author: Option<String>,
    #[arg(long)]
    keyword: Option<String>,
}

#[derive(Args)]
struct CommentAddArgs {
    #[arg(short = 'i', long = "interactive", conflicts_with_all = ["on", "subject", "author", "body"])]
    interactive: bool,
    #[arg(long = "on")]
    on: Option<String>,
    #[arg(long)]
    subject: Option<String>,
    #[arg(long)]
    author: Option<String>,
    #[arg(long)]
    body: Option<String>,
}

/// Shared by the raw `query` verb everywhere it appears (top-level `query
/// run` and each entity's own `query`) -- one args shape, one implementation
/// in [`run_query_args`], over the library's `query_goal`/`query_predicate`.
#[derive(Args)]
struct QueryArgs {
    #[arg(conflicts_with = "goal")]
    predicate: Option<String>,
    #[arg(long = "bind", value_name = "POSITION=VALUE")]
    bind: Vec<String>,
    #[arg(long)]
    goal: Option<String>,
    #[arg(long, value_delimiter = ',')]
    select: Vec<String>,
}

#[derive(Subcommand)]
enum QueryCommand {
    Run(QueryArgs),
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
    #[arg(short = 'i', long = "interactive", conflicts_with_all = ["body", "reviewers", "requesters", "target", "status"])]
    interactive: bool,
    #[arg(long)]
    body: Option<String>,
    #[arg(long, value_enum)]
    status: Option<StatusArg>,
    #[arg(long = "reviewer")]
    reviewers: Vec<String>,
    #[arg(long = "requester")]
    requesters: Vec<String>,
    #[arg(long, value_name = "TARGET")]
    target: Option<String>,
}

#[derive(Args)]
struct IssueEditArgs {
    id: String,
    #[arg(short = 'i', long = "interactive", conflicts_with_all = ["title", "body", "status"])]
    interactive: bool,
    #[arg(long)]
    title: Option<String>,
    #[arg(long)]
    body: Option<String>,
    #[arg(long, value_enum)]
    status: Option<StatusArg>,
}

#[derive(Args)]
struct ReviewEditArgs {
    id: String,
    #[arg(short = 'i', long = "interactive", conflicts_with_all = ["body", "target", "status"])]
    interactive: bool,
    #[arg(long)]
    body: Option<String>,
    #[arg(long)]
    target: Option<String>,
    #[arg(long, value_enum)]
    status: Option<StatusArg>,
}

#[derive(Args)]
struct CommentEditArgs {
    id: String,
    #[arg(short = 'i', long = "interactive", conflicts_with_all = ["body"])]
    interactive: bool,
    #[arg(long)]
    body: Option<String>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let mut repo = gix::discover(".").context("not inside a git repository")?;
    // Every forge command reads entities, and a query scans a whole kind:
    // the same schema objects come back thousands of times per run, so give
    // gix's object cache -- off unless asked for -- something to hold them.
    repo.object_cache_size_if_unset(4 * 1024 * 1024);

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
        IssueCommand::Add(args) => {
            let interactive = resolve_interactive(
                args.interactive,
                args.title.is_none()
                    && args.body.is_none()
                    && args.status.is_none()
                    && args.labels.is_empty()
                    && args.assignees.is_empty()
                    && args.reporters.is_empty(),
            )?;
            let fields = if interactive {
                prompt_issue_fields(repo)?
            } else {
                IssueFields {
                    title: args.title.unwrap_or_default(),
                    body: args
                        .body
                        .context("--body is required unless running interactively")?,
                    labels: args.labels,
                    assignees: args.assignees,
                    reporters: args.reporters,
                }
            };
            let issue = Issue {
                id: String::new(),
                status: args
                    .status
                    .unwrap_or(StatusArg::Open)
                    .as_status()
                    .as_str()
                    .to_owned(),
                title: fields.title,
                body: fields.body,
                labels: fields.labels,
                assignees: fields.assignees,
                reporters: fields.reporters,
                edit: None,
            };
            println!("{}", issue.create_in_repo(repo)?);
        }
        IssueCommand::Edit(args) => {
            let id = resolve_issue_show_id(repo, &args.id)?
                .with_context(|| format!("no issue {}", args.id))?;
            let mut issue =
                Issue::load_from_repo(repo, &id)?.with_context(|| format!("no issue {id}"))?;
            let no_args_supplied =
                args.title.is_none() && args.body.is_none() && args.status.is_none();
            let interactive = resolve_interactive(args.interactive, no_args_supplied)?;
            if interactive {
                let (title, body, status) = prompt_issue_edit_fields(&issue)?;
                if let Some(title) = title {
                    issue.title = title;
                }
                if let Some(body) = body {
                    issue.body = body;
                }
                if let Some(status) = status {
                    issue.status = status;
                }
                issue.edit = None;
            } else {
                if no_args_supplied {
                    bail!("--title, --body, or --status is required unless running interactively");
                }
                if let Some(title) = args.title {
                    issue.title = title;
                }
                if let Some(body) = args.body {
                    issue.body = body;
                }
                if let Some(status) = args.status {
                    issue.status = status.as_status().as_str().to_owned();
                }
                issue.edit = None;
            }
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
        IssueCommand::Search(args) => {
            let ids =
                gix_forge::search_issue(repo, args.assignee.as_deref(), args.keyword.as_deref())?;
            print_id_list("issues", &ids);
        }
        IssueCommand::Query(args) => run_query_args(repo, args)?,
    }
    Ok(())
}

fn run_comment(repo: &gix::Repository, command: CommentCommand) -> Result<()> {
    match command {
        CommentCommand::Add(args) => {
            let interactive = resolve_interactive(
                args.interactive,
                args.on.is_none()
                    && args.subject.is_none()
                    && args.author.is_none()
                    && args.body.is_none(),
            )?;
            let (kind, subject_id, author, body) = if interactive {
                prompt_comment_fields(repo)?
            } else {
                let kind = args
                    .on
                    .as_deref()
                    .context("--on is required unless running interactively")?;
                let subject_input = args
                    .subject
                    .context("--subject is required unless running interactively")?;
                let subject_id = resolve_comment_subject_id(repo, kind, &subject_input)?
                    .with_context(|| format!("no {kind} {subject_input}"))?;
                (
                    kind.to_owned(),
                    subject_id,
                    args.author
                        .context("--author is required unless running interactively")?,
                    args.body
                        .context("--body is required unless running interactively")?,
                )
            };
            let id = Comment::create_under(repo, &kind, &subject_id, &author, &body, None)?;
            println!("{id}");
        }
        CommentCommand::Edit(args) => {
            let id = resolve_comment_show_id(repo, &args.id)?
                .with_context(|| format!("no comment {}", args.id))?;
            let mut comment =
                Comment::load_from_repo(repo, &id)?.with_context(|| format!("no comment {id}"))?;
            let no_args_supplied = args.body.is_none();
            let interactive = resolve_interactive(args.interactive, no_args_supplied)?;
            if interactive {
                comment.body = prompt_body(&comment.body)?;
            } else {
                if no_args_supplied {
                    bail!("--body is required unless running interactively");
                }
                if let Some(body) = args.body {
                    comment.body = body;
                }
            }
            comment.edit = None;
            println!("{}", comment.save_in_repo(repo)?);
        }
        CommentCommand::Show { id } => {
            let id =
                resolve_comment_show_id(repo, &id)?.with_context(|| format!("no comment {id}"))?;
            let comment =
                Comment::load_from_repo(repo, &id)?.with_context(|| format!("no comment {id}"))?;
            print_comment(&comment);
        }
        CommentCommand::List => print_comment_list(repo)?,
        CommentCommand::Log { id } => {
            print_log("comment", &id, repo, Comment::history(repo, &id)?)?;
        }
        CommentCommand::Rm { id } => {
            if !Comment::delete(repo, &id)? {
                bail!("no comment {id}");
            }
        }
        CommentCommand::Search(args) => {
            let ids =
                gix_forge::search_comment(repo, args.author.as_deref(), args.keyword.as_deref())?;
            print_id_list("comments", &ids);
        }
        CommentCommand::Query(args) => run_query_args(repo, args)?,
    }
    Ok(())
}

fn resolve_comment_subject_id(
    repo: &gix::Repository,
    kind: &str,
    input: &str,
) -> Result<Option<String>> {
    match kind {
        Issue::KIND => resolve_issue_show_id(repo, input),
        Review::KIND => resolve_review_show_id(repo, input),
        _ => Ok(Some(input.to_owned())),
    }
}

fn resolve_comment_show_id(repo: &gix::Repository, id: &str) -> Result<Option<String>> {
    resolve_show_id("comment", id, &Comment::list(repo)?)
}

fn prompt_comment_fields(repo: &gix::Repository) -> Result<(String, String, String, String)> {
    require_terminal_for_interactive()?;
    let kinds = ["issue", "review"];
    let choice = Select::with_theme(&interactive_theme())
        .with_prompt("Comment on")
        .items(&kinds)
        .default(0)
        .interact()?;
    let kind = if choice == 0 {
        Issue::KIND.to_owned()
    } else {
        Review::KIND.to_owned()
    };
    let subject_input = prompt_required_text(&format!("{} id", kinds[choice]))?;
    let subject_id = resolve_comment_subject_id(repo, &kind, &subject_input)?
        .with_context(|| format!("no {} {subject_input}", kinds[choice]))?;
    let author = prompt_required_text("Author")?;
    let body = prompt_body("")?;
    Ok((kind, subject_id, author, body))
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
    require_terminal_for_interactive()?;

    Ok(Confirm::with_theme(&interactive_theme())
        .with_prompt(format!("install {prompt}?"))
        .default(true)
        .interact()?)
}

fn resolve_interactive(explicit: bool, no_args_supplied: bool) -> Result<bool> {
    if explicit {
        require_terminal_for_interactive()?;
        return Ok(true);
    }
    Ok(no_args_supplied && std::io::stdin().is_terminal())
}

fn interactive_theme() -> ColorfulTheme {
    ColorfulTheme::default()
}

fn prompt_optional_text(prompt: &str) -> Result<String> {
    require_terminal_for_interactive()?;
    Ok(Input::with_theme(&interactive_theme())
        .with_prompt(prompt)
        .allow_empty(true)
        .interact_text()?)
}

fn prompt_required_text(prompt: &str) -> Result<String> {
    require_terminal_for_interactive()?;
    Ok(Input::with_theme(&interactive_theme())
        .with_prompt(prompt)
        .validate_with(|input: &String| -> Result<(), &str> {
            if input.trim().is_empty() {
                Err("required")
            } else {
                Ok(())
            }
        })
        .interact_text()?)
}

fn prompt_text_with_default(prompt: &str, default: &str) -> Result<String> {
    require_terminal_for_interactive()?;
    Ok(Input::with_theme(&interactive_theme())
        .with_prompt(prompt)
        .default(default.to_owned())
        .interact_text()?)
}

/// Prompt for a (typically long-form) body via the user's `$VISUAL`/`$EDITOR`.
fn prompt_body(initial: &str) -> Result<String> {
    require_terminal_for_interactive()?;
    let edited = InteractiveEditor::new()
        .edit(initial)
        .context("failed to launch editor")?
        .context("cancelled: body was not saved")?;
    let value = edited.trim_end_matches(['\r', '\n']).to_owned();
    if value.trim().is_empty() {
        bail!("body is required");
    }
    Ok(value)
}

fn prompt_status(current: Status) -> Result<Status> {
    require_terminal_for_interactive()?;
    let items = ["Open", "Closed"];
    let default_index = match current {
        Status::Open => 0,
        Status::Closed => 1,
    };
    let choice = Select::with_theme(&interactive_theme())
        .with_prompt("Status")
        .items(&items)
        .default(default_index)
        .interact()?;
    Ok(if choice == 0 {
        Status::Open
    } else {
        Status::Closed
    })
}

/// Pick zero or more values for a repeated field (labels, assignees, …). When
/// `known` values exist (gathered from other entities already in the repo)
/// they're offered as a multi-select picker; either way the user can also
/// type in new, comma-separated values.
fn prompt_multi_values(label: &str, known: &[String]) -> Result<Vec<String>> {
    require_terminal_for_interactive()?;
    let theme = interactive_theme();

    let mut selected: Vec<String> = if known.is_empty() {
        Vec::new()
    } else {
        MultiSelect::with_theme(&theme)
            .with_prompt(format!("{label} (space to toggle, enter to confirm)"))
            .items(known)
            .interact()?
            .into_iter()
            .map(|index| known[index].clone())
            .collect()
    };

    let extra: String = Input::with_theme(&theme)
        .with_prompt(format!("Add {label} (comma-separated, optional)"))
        .allow_empty(true)
        .interact_text()?;
    for value in parse_csv_input(&extra) {
        if !selected.contains(&value) {
            selected.push(value);
        }
    }
    Ok(selected)
}

fn known_issue_values(
    repo: &gix::Repository,
    select: impl Fn(&Issue) -> &[String],
) -> Result<Vec<String>> {
    let mut values = std::collections::BTreeSet::new();
    for id in Issue::list(repo)? {
        if let Some(issue) = Issue::load_from_repo(repo, &id)? {
            values.extend(select(&issue).iter().cloned());
        }
    }
    Ok(values.into_iter().collect())
}

fn known_review_values(
    repo: &gix::Repository,
    select: impl Fn(&Review) -> &[String],
) -> Result<Vec<String>> {
    let mut values = std::collections::BTreeSet::new();
    for id in Review::list(repo)? {
        if let Some(review) = Review::load_from_repo(repo, &id)? {
            values.extend(select(&review).iter().cloned());
        }
    }
    Ok(values.into_iter().collect())
}

struct IssueFields {
    title: String,
    body: String,
    labels: Vec<String>,
    assignees: Vec<String>,
    reporters: Vec<String>,
}

fn prompt_issue_fields(repo: &gix::Repository) -> Result<IssueFields> {
    let title = prompt_optional_text("Title")?;
    let body = prompt_body("")?;
    let labels = prompt_multi_values("Labels", &known_issue_values(repo, |issue| &issue.labels)?)?;
    let assignees = prompt_multi_values(
        "Assignees",
        &known_issue_values(repo, |issue| &issue.assignees)?,
    )?;
    let reporters = prompt_multi_values(
        "Reporters",
        &known_issue_values(repo, |issue| &issue.reporters)?,
    )?;
    Ok(IssueFields {
        title,
        body,
        labels,
        assignees,
        reporters,
    })
}

fn prompt_issue_edit_fields(
    issue: &Issue,
) -> Result<(Option<String>, Option<String>, Option<String>)> {
    let fields = ["Title", "Body", "Status"];
    let selected = MultiSelect::with_theme(&interactive_theme())
        .with_prompt("What would you like to edit?")
        .items(&fields)
        .interact()?;
    if selected.is_empty() {
        bail!("select at least one field");
    }

    let mut title = None;
    let mut body = None;
    let mut status = None;
    for index in selected {
        match index {
            0 => title = Some(prompt_text_with_default("Title", &issue.title)?),
            1 => body = Some(prompt_body(&issue.body)?),
            2 => {
                let current = Status::parse(&issue.status).unwrap_or(Status::Open);
                status = Some(prompt_status(current)?.as_str().to_owned());
            }
            _ => unreachable!(),
        }
    }
    Ok((title, body, status))
}

fn prompt_review_fields(
    repo: &gix::Repository,
) -> Result<(String, Vec<String>, Vec<String>, String)> {
    let body = prompt_body("")?;
    let reviewers = prompt_multi_values(
        "Reviewers",
        &known_review_values(repo, |review| &review.reviewers)?,
    )?;
    let requesters = prompt_multi_values(
        "Requesters",
        &known_review_values(repo, |review| &review.requesters)?,
    )?;
    let target = prompt_required_text("Target (e.g. commit:<oid>, blob:<path>:<oid>)")?;
    Ok((body, reviewers, requesters, target))
}

fn prompt_review_edit_fields(
    review: &Review,
) -> Result<(Option<String>, Option<String>, Option<String>)> {
    let fields = ["Body", "Target", "Status"];
    let selected = MultiSelect::with_theme(&interactive_theme())
        .with_prompt("What would you like to edit?")
        .items(&fields)
        .interact()?;
    if selected.is_empty() {
        bail!("select at least one field");
    }

    let mut body = None;
    let mut target = None;
    let mut status = None;
    for index in selected {
        match index {
            0 => body = Some(prompt_body(&review.body)?),
            1 => {
                let current = review.target.to_string();
                target = Some(prompt_text_with_default("Target", &current)?);
            }
            2 => {
                let current = Status::parse(&review.status).unwrap_or(Status::Open);
                status = Some(prompt_status(current)?.as_str().to_owned());
            }
            _ => unreachable!(),
        }
    }
    Ok((body, target, status))
}

fn parse_csv_input(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn require_terminal_for_interactive() -> Result<()> {
    if std::io::stdin().is_terminal() {
        Ok(())
    } else {
        bail!("--interactive requires a terminal")
    }
}

fn run_query(repo: &gix::Repository, command: QueryCommand) -> Result<()> {
    match command {
        QueryCommand::Run(args) => run_query_args(repo, args)?,
        QueryCommand::Assignee { name } => {
            let ids = gix_forge::search_assignee(repo, &name)?;
            print_id_list(&format!("issues assigned to {name}"), &ids);
        }
        QueryCommand::Reviewer { name } => {
            let ids = gix_forge::search_reviewer(repo, &name)?;
            print_id_list(&format!("reviews by reviewer {name}"), &ids);
        }
        QueryCommand::Requester { name } => {
            let ids = gix_forge::search_requester(repo, &name)?;
            print_id_list(&format!("reviews by requester {name}"), &ids);
        }
        QueryCommand::Keyword { value } => {
            let hits = gix_forge::search_keyword(repo, &value)?;
            let matches: Vec<String> = hits
                .iter()
                .map(|hit| format!("{} {}", hit.kind.as_str(), hit.id))
                .collect();
            print_bulleted_section(&format!("matches for \"{value}\""), &matches);
        }
        QueryCommand::Find {
            assignee,
            reviewer,
            requester,
            keyword,
            title,
        } => {
            let needle = keyword.or(title);
            if assignee.is_none() && reviewer.is_none() && requester.is_none() && needle.is_none() {
                bail!(
                    "query find requires at least one filter: --assignee, --reviewer, --requester, --keyword, or --title"
                );
            }

            let hits = gix_forge::search_find(
                repo,
                assignee.as_deref(),
                reviewer.as_deref(),
                requester.as_deref(),
                needle.as_deref(),
            )?;
            let matches: Vec<String> = hits
                .iter()
                .map(|hit| format!("{} {}", hit.kind.as_str(), hit.id))
                .collect();
            print_bulleted_section("query matches", &matches);
        }
    }
    Ok(())
}

/// The raw predicate/goal query -- shared by top-level `query run` and every
/// entity's own `query` verb, all four of which take the same [`QueryArgs`]
/// and run it exactly the same way.
fn run_query_args(repo: &gix::Repository, args: QueryArgs) -> Result<()> {
    let QueryArgs {
        predicate,
        bind,
        goal,
        select,
    } = args;
    match (predicate, goal) {
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
        (None, None) => bail!("query requires either a predicate or --goal"),
        (Some(_), Some(_)) => unreachable!("clap rejects predicate with --goal"),
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
        ReviewCommand::Add(args) => {
            let interactive = resolve_interactive(
                args.interactive,
                args.body.is_none()
                    && args.status.is_none()
                    && args.reviewers.is_empty()
                    && args.requesters.is_empty()
                    && args.target.is_none(),
            )?;
            let (body, reviewers, requesters, target) = if interactive {
                prompt_review_fields(repo)?
            } else {
                (
                    args.body
                        .context("--body is required unless running interactively")?,
                    args.reviewers,
                    args.requesters,
                    args.target
                        .context("--target is required unless running interactively")?,
                )
            };
            let review = Review {
                id: String::new(),
                status: args
                    .status
                    .unwrap_or(StatusArg::Open)
                    .as_status()
                    .as_str()
                    .to_owned(),
                body,
                reviewers,
                requesters,
                target: ReviewTarget::parse(&target)?,
                edit: None,
            };
            println!("{}", review.create_in_repo(repo)?);
        }
        ReviewCommand::Edit(args) => {
            let id = resolve_review_show_id(repo, &args.id)?
                .with_context(|| format!("no review {}", args.id))?;
            let mut review =
                Review::load_from_repo(repo, &id)?.with_context(|| format!("no review {id}"))?;
            let no_args_supplied =
                args.body.is_none() && args.target.is_none() && args.status.is_none();
            let interactive = resolve_interactive(args.interactive, no_args_supplied)?;
            if interactive {
                let (body, target, status) = prompt_review_edit_fields(&review)?;
                if let Some(body) = body {
                    review.body = body;
                }
                if let Some(target) = target {
                    review.target = ReviewTarget::parse(&target)?;
                }
                if let Some(status) = status {
                    review.status = status;
                }
                review.edit = None;
            } else {
                if no_args_supplied {
                    bail!("--body, --target, or --status is required unless running interactively");
                }
                if let Some(body) = args.body {
                    review.body = body;
                }
                if let Some(target) = args.target {
                    review.target = ReviewTarget::parse(&target)?;
                }
                if let Some(status) = args.status {
                    review.status = status.as_status().as_str().to_owned();
                }
                review.edit = None;
            }
            println!("{}", review.save_in_repo(repo)?);
        }
        ReviewCommand::Show { id } => {
            let id =
                resolve_review_show_id(repo, &id)?.with_context(|| format!("no review {id}"))?;
            let review =
                Review::load_from_repo(repo, &id)?.with_context(|| format!("no review {id}"))?;
            print_review(&review);
        }
        ReviewCommand::List => print_review_list(repo)?,
        ReviewCommand::Log { id } => print_log("review", &id, repo, Review::history(repo, &id)?)?,
        ReviewCommand::Rm { id } => {
            if !Review::delete(repo, &id)? {
                bail!("no review {id}");
            }
        }
        ReviewCommand::Search(args) => {
            let ids = gix_forge::search_review(
                repo,
                args.reviewer.as_deref(),
                args.requester.as_deref(),
                args.keyword.as_deref(),
            )?;
            print_id_list("reviews", &ids);
        }
        ReviewCommand::Query(args) => run_query_args(repo, args)?,
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

fn format_status(status: &str) -> String {
    match Status::parse(status) {
        Some(Status::Open) => "Open".green().bold().to_string(),
        Some(Status::Closed) => "Closed".dimmed().bold().to_string(),
        None => status.to_owned(),
    }
}

fn print_id_list(title: &str, ids: &[String]) {
    print_bulleted_section(title, ids);
}

/// Every entity's own `list` command loads each id and formats a table row
/// the same way -- `to_row` supplies only what's genuinely specific to the
/// kind (its two non-status columns, and its status if it has one).
fn entity_rows<T: EntityOps>(
    repo: &gix::Repository,
    to_row: impl Fn(&T) -> (String, String, String),
) -> Result<Vec<Vec<String>>> {
    let mut rows = Vec::new();
    for id in T::list(repo)? {
        let Some(entity) = T::load_from_repo(repo, &id)? else {
            continue;
        };
        let (col2, col3, status) = to_row(&entity);
        rows.push(vec![
            format!("#{id}"),
            col2,
            col3,
            status,
            updated_relative(repo, T::history(repo, &id)?)?,
        ]);
    }
    Ok(rows)
}

fn print_issue_list(repo: &gix::Repository) -> Result<()> {
    let rows = entity_rows::<Issue>(repo, |issue| {
        (
            title_or_untitled(&issue.title).to_owned(),
            join_values_or_none(&issue.labels),
            issue.status.clone(),
        )
    })?;
    print_entity_table("issues", "TITLE", "LABELS", rows);
    Ok(())
}

fn print_review_list(repo: &gix::Repository) -> Result<()> {
    let rows = entity_rows::<Review>(repo, |review| {
        (
            review.target.to_string(),
            join_values_or_none(&review.reviewers),
            review.status.clone(),
        )
    })?;
    print_entity_table("reviews", "TARGET", "REVIEWERS", rows);
    Ok(())
}

fn print_comment_list(repo: &gix::Repository) -> Result<()> {
    let rows = entity_rows::<Comment>(repo, |comment| {
        (
            comment.subject.clone(),
            comment.author.clone(),
            "-".to_owned(),
        )
    })?;
    print_entity_table("comments", "SUBJECT", "AUTHOR", rows);
    Ok(())
}

fn print_entity_table(kind_plural: &str, col2: &str, col3: &str, rows: Vec<Vec<String>>) {
    if rows.is_empty() {
        println!("No {kind_plural}");
        return;
    }

    let mut table = Table::new();
    table
        .load_preset(NOTHING)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            Cell::new("ID").add_attribute(Attribute::Bold),
            Cell::new(col2).add_attribute(Attribute::Bold),
            Cell::new(col3).add_attribute(Attribute::Bold),
            Cell::new("STATUS").add_attribute(Attribute::Bold),
            Cell::new("UPDATED").add_attribute(Attribute::Bold),
        ]);

    for row in rows {
        table.add_row(vec![
            Cell::new(color_id(&row[0])).set_alignment(CellAlignment::Left),
            Cell::new(&row[1]).set_alignment(CellAlignment::Left),
            Cell::new(&row[2]).set_alignment(CellAlignment::Left),
            Cell::new(&row[3]).set_alignment(CellAlignment::Left),
            Cell::new(&row[4]).set_alignment(CellAlignment::Left),
        ]);
    }

    println!("{table}");
}

fn updated_relative(repo: &gix::Repository, history: Vec<gix::ObjectId>) -> Result<String> {
    let Some(oid) = history.first() else {
        return Ok("(unknown)".to_owned());
    };

    let commit = repo.find_commit(*oid)?;
    let time = commit.time()?;
    let when = time.format(gix::date::time::format::ISO8601)?;
    Ok(relative_time_from_unix_seconds(time.seconds).unwrap_or(when))
}

fn relative_time_from_unix_seconds(seconds: i64) -> Option<String> {
    let then = DateTime::from_timestamp(seconds, 0)?;
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

fn print_issue(issue: &Issue) {
    let mut meta = vec![format_status(&issue.status)];
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

    print_show_doc(ShowDoc {
        kind: "issue",
        id: &issue.id,
        title: Some(&issue.title),
        meta,
        body: &issue.body,
    });
}

fn print_review(review: &Review) {
    let mut meta = vec![format_status(&review.status)];
    meta.push(format!(
        "reviewers: {}",
        join_values_or_none(&review.reviewers)
    ));
    meta.push(format!(
        "requesters: {}",
        join_values_or_none(&review.requesters)
    ));
    meta.push(format!("target: {}", review.target));
    if let Some(edit) = &review.edit {
        meta.push(format!("edit: {edit}"));
    }

    print_show_doc(ShowDoc {
        kind: "review",
        id: &review.id,
        title: None,
        meta,
        body: &review.body,
    });
}

fn print_comment(comment: &Comment) {
    let mut meta = vec![
        format!("subject: {}", comment.subject),
        format!("author: {}", comment.author),
    ];
    if let Some(edit) = &comment.edit {
        meta.push(format!("edit: {edit}"));
    }

    print_show_doc(ShowDoc {
        kind: "comment",
        id: &comment.id,
        title: None,
        meta,
        body: &comment.body,
    });
}

struct ShowDoc<'a> {
    kind: &'a str,
    id: &'a str,
    title: Option<&'a str>,
    meta: Vec<String>,
    body: &'a str,
}

fn print_show_doc(doc: ShowDoc<'_>) {
    if let Some(title) = doc.title {
        println!(
            "{} {}",
            title_or_untitled(title).bold(),
            format!("#{}", doc.id).yellow()
        );
    } else {
        println!("{} {}", doc.kind.bold(), doc.id.yellow());
    }

    if !doc.meta.is_empty() {
        let separator = format!(" {} ", "•".dimmed());
        println!("{}", doc.meta.join(&separator));
        println!();
    }

    if doc.body.trim().is_empty() {
        println!("{}", color_empty_marker("(none)"));
        return;
    }

    print_rendered(&render_asciidoc_terminal(doc.body));
}

fn title_or_untitled(title: &str) -> &str {
    if title.trim().is_empty() {
        "(untitled)"
    } else {
        title
    }
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
