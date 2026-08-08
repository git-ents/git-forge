//! `git-forge`: A Git subcommand for store, anchor, and query.

use std::io::IsTerminal;
use std::sync::Arc;

#[cfg(unix)]
use serde_json::{Value, json};

use acdc_converters_core::{Converter as _, Diagnostics, Options as ConvertOptions, WarningSource};
use acdc_converters_terminal::Processor as TerminalProcessor;
use acdc_parser::{Options as ParseOptions, parse as parse_asciidoc};
use anyhow::{Context, Result, bail};
use chrono::{DateTime, Utc};
use clap::{Args, Parser, Subcommand, ValueEnum};
use comfy_table::{Attribute, Cell, CellAlignment, ContentArrangement, Table, presets::NOTHING};
use dialoguer::{
    Completion, Confirm, Editor as InteractiveEditor, Input, MultiSelect, Select,
    theme::ColorfulTheme,
};
use gix_forge::{
    Authorization, Comment, Entity, EntityOps, HitKind, Issue, Member, Principal, QueryValue,
    Review, ReviewTarget, Status,
};
use owo_colors::OwoColorize;

#[derive(Parser)]
#[command(name = "git-forge", about = "Forge software on Git", version)]
struct Cli {
    /// Member identity used for mutations. Reads do not require an identity.
    #[arg(long = "as", global = true, value_name = "MEMBER_ID")]
    as_member: Option<String>,
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
    Member(MemberCommand),
    #[command(subcommand)]
    Comment(CommentCommand),
    #[command(subcommand)]
    Query(QueryCommand),
    Install(InstallArgs),
    Uninstall(UninstallArgs),
    Ui(UiArgs),
    UiStop,
    UiStatus,
}

#[derive(Args)]
struct InstallArgs {
    #[arg(short = 'i', long = "interactive")]
    interactive: bool,
}

#[derive(Args)]
struct UninstallArgs {
    #[arg(short = 'i', long = "interactive")]
    interactive: bool,
}

#[derive(Args)]
struct UiArgs {
    #[arg(long)]
    detach: bool,
    #[arg(long)]
    open: bool,
    #[arg(long, default_value = "5050")]
    port: Option<u16>,
    #[arg(long, default_value = "git-forge.localhost")]
    host: String,
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
enum MemberCommand {
    #[command(alias = "new")]
    Add(MemberAddArgs),
    Edit(MemberEditArgs),
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
struct MemberAddArgs {
    #[arg(short = 'i', long = "interactive", conflicts_with_all = ["signing_key", "role"])]
    interactive: bool,
    #[arg(long = "signing-key")]
    signing_key: Option<String>,
    #[arg(long)]
    role: Option<String>,
    #[arg(long)]
    bootstrap: bool,
}

#[derive(Args)]
struct MemberEditArgs {
    id: String,
    #[arg(short = 'i', long = "interactive", conflicts_with_all = ["signing_key", "role"])]
    interactive: bool,
    #[arg(long = "signing-key")]
    signing_key: Option<String>,
    #[arg(long)]
    role: Option<String>,
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
    let as_member = cli.as_member;
    let authorization = resolve_authorization(&repo, as_member.as_deref())?;

    match cli.command {
        Command::Issue(command) => run_issue(&repo, &authorization, command)?,
        Command::Review(command) => run_review(&repo, &authorization, command)?,
        Command::Member(command) => {
            run_member(&repo, &authorization, as_member.as_deref(), command)?
        }
        Command::Comment(command) => run_comment(&repo, &authorization, command)?,
        Command::Query(command) => run_query(&repo, command)?,
        Command::Install(args) => run_install(&repo, &authorization, args)?,
        Command::Uninstall(args) => run_uninstall(&repo, args)?,
        Command::Ui(args) => run_ui(&repo, args)?,
        Command::UiStop => run_ui_stop(&repo)?,
        Command::UiStatus => run_ui_status(&repo)?,
    }

    Ok(())
}

fn run_ui(repo: &gix::Repository, args: UiArgs) -> Result<()> {
    if args.detach {
        return run_ui_detached(repo, args);
    }

    let repo_path = repo.path().to_owned();
    let member_id = resolve_ui_member_id(repo);
    let host = args.host;
    let open = args.open;
    let port = args.port;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("failed to create the UI runtime")?;

    runtime.block_on(async move {
        let repo = gix::ThreadSafeRepository::open(repo_path)
            .context("failed to open the repository for the UI")?;
        let router = git_forge_ui::build_router(Arc::new(repo), member_id).await;
        let listener = tokio::net::TcpListener::bind((host.as_str(), port.unwrap_or(0)))
            .await
            .with_context(|| format!("failed to bind the UI to {host}:{:?}", port))?;
        let port = listener
            .local_addr()
            .context("failed to determine the UI listener address")?
            .port();

        let url = ui_url(&host, port);
        println!("{url}");
        if open {
            open_url(&url)?;
        }
        topcoat::serve(listener, router)
            .await
            .context("the UI server failed")?;
        Ok(())
    })
}

fn ui_url(host: &str, port: u16) -> String {
    let host = if host.starts_with('[') && host.ends_with(']') {
        host.to_owned()
    } else if host.contains(':') {
        format!("[{host}]")
    } else {
        host.to_owned()
    };
    format!("https://{host}:{port}")
}

#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
fn open_url(url: &str) -> Result<()> {
    #[cfg(target_os = "macos")]
    let mut command = std::process::Command::new("open");
    #[cfg(target_os = "linux")]
    let mut command = std::process::Command::new("xdg-open");
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = std::process::Command::new("cmd");
        command.args(["/C", "start", "", url]);
        command
    };
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    command.arg(url);
    let status = command.status().context("failed to open the UI URL")?;
    if !status.success() {
        bail!("failed to open the UI URL: {url}");
    }
    Ok(())
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn open_url(_url: &str) -> Result<()> {
    bail!("opening UI URLs is unsupported on this platform");
}

#[cfg(unix)]
fn run_ui_detached(repo: &gix::Repository, args: UiArgs) -> Result<()> {
    use std::os::unix::process::CommandExt;
    use std::process::{Command, Stdio};

    let port = resolve_ui_port(&args.host, args.port)?;
    let log_path = repo.git_dir().join("git-forge-ui.log");
    let stdout = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .with_context(|| format!("failed to open UI log {}", log_path.display()))?;
    let stderr = stdout
        .try_clone()
        .with_context(|| format!("failed to prepare UI log {}", log_path.display()))?;
    let executable =
        std::env::current_exe().context("failed to locate the git-forge executable")?;
    let mut command = Command::new(executable);
    command
        .arg("ui")
        .arg("--host")
        .arg(&args.host)
        .arg("--port")
        .arg(port.to_string());
    if args.open {
        command.arg("--open");
    }
    command
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr));
    unsafe {
        command.pre_exec(|| {
            if libc::setsid() == -1 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }

    let child = command
        .spawn()
        .with_context(|| "failed to start the detached UI")?;
    let url = ui_url(&args.host, port);
    let pidfile = repo.git_dir().join("git-forge-ui.pid");
    let contents = serde_json::to_vec_pretty(&json!({
        "pid": child.id(),
        "host": args.host,
        "port": port,
        "url": url,
    }))
    .context("failed to encode the UI pidfile")?;
    std::fs::write(&pidfile, contents)
        .with_context(|| format!("failed to write UI pidfile {}", pidfile.display()))?;
    println!("{url}");
    Ok(())
}

#[cfg(not(unix))]
fn run_ui_detached(_repo: &gix::Repository, _args: UiArgs) -> Result<()> {
    bail!("--detach is not supported on Windows; run `git forge ui` in the foreground")
}

#[cfg(unix)]
fn resolve_ui_port(host: &str, port: Option<u16>) -> Result<u16> {
    let listener = std::net::TcpListener::bind((host, port.unwrap_or(0)))
        .with_context(|| format!("failed to reserve the UI address {host}:{:?}", port))?;
    listener
        .local_addr()
        .context("failed to determine the reserved UI port")
        .map(|address| address.port())
}

#[cfg(unix)]
struct UiState {
    pid: i32,
    url: String,
}

#[cfg(unix)]
fn read_ui_state(repo: &gix::Repository) -> Result<Option<UiState>> {
    let pidfile = repo.git_dir().join("git-forge-ui.pid");
    let contents = match std::fs::read_to_string(&pidfile) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(error)
                .with_context(|| format!("failed to read UI pidfile {}", pidfile.display()));
        }
    };
    let value: Value = serde_json::from_str(&contents)
        .with_context(|| format!("invalid JSON in UI pidfile {}", pidfile.display()))?;
    let pid = value
        .get("pid")
        .and_then(Value::as_i64)
        .with_context(|| format!("UI pidfile {} has no integer pid", pidfile.display()))?;
    let pid = i32::try_from(pid)
        .with_context(|| format!("UI pidfile {} has an invalid pid", pidfile.display()))?;
    if pid <= 0 {
        bail!("UI pidfile {} has an invalid pid", pidfile.display());
    }
    value
        .get("host")
        .and_then(Value::as_str)
        .with_context(|| format!("UI pidfile {} has no host", pidfile.display()))?;
    let port = value
        .get("port")
        .and_then(Value::as_u64)
        .with_context(|| format!("UI pidfile {} has no integer port", pidfile.display()))?;
    u16::try_from(port)
        .with_context(|| format!("UI pidfile {} has an invalid port", pidfile.display()))?;
    let url = value
        .get("url")
        .and_then(Value::as_str)
        .with_context(|| format!("UI pidfile {} has no URL", pidfile.display()))?
        .to_owned();
    Ok(Some(UiState { pid, url }))
}

#[cfg(unix)]
fn run_ui_stop(repo: &gix::Repository) -> Result<()> {
    let Some(state) = read_ui_state(repo)? else {
        println!("not running");
        return Ok(());
    };

    let signal_error = if unsafe { libc::kill(state.pid as libc::pid_t, libc::SIGTERM) } == 0 {
        None
    } else {
        Some(std::io::Error::last_os_error())
    };
    let pidfile = repo.git_dir().join("git-forge-ui.pid");
    std::fs::remove_file(&pidfile)
        .with_context(|| format!("failed to remove UI pidfile {}", pidfile.display()))?;

    match signal_error {
        None => println!("stopped {}", state.url),
        Some(error) if error.raw_os_error() == Some(libc::ESRCH) => println!("not running"),
        Some(error) => bail!("failed to stop UI process {}: {error}", state.pid),
    }
    Ok(())
}

#[cfg(not(unix))]
fn run_ui_stop(_repo: &gix::Repository) -> Result<()> {
    bail!("ui-stop is not supported on Windows")
}

#[cfg(unix)]
fn run_ui_status(repo: &gix::Repository) -> Result<()> {
    let Some(state) = read_ui_state(repo)? else {
        println!("not running");
        return Ok(());
    };

    let alive = if unsafe { libc::kill(state.pid as libc::pid_t, 0) } == 0 {
        true
    } else {
        match std::io::Error::last_os_error().raw_os_error() {
            Some(libc::ESRCH) => false,
            Some(libc::EPERM) => true,
            _ => {
                return Err(std::io::Error::last_os_error())
                    .with_context(|| format!("failed to check UI process {}", state.pid));
            }
        }
    };
    if alive {
        println!("{}", state.url);
        return Ok(());
    }

    let pidfile = repo.git_dir().join("git-forge-ui.pid");
    std::fs::remove_file(&pidfile)
        .with_context(|| format!("failed to remove stale UI pidfile {}", pidfile.display()))?;
    println!("not running");
    Ok(())
}

#[cfg(not(unix))]
fn run_ui_status(_repo: &gix::Repository) -> Result<()> {
    bail!("ui-status is not supported on Windows")
}

fn resolve_authorization(repo: &gix::Repository, member_id: Option<&str>) -> Result<Authorization> {
    let Some(member_id) = member_id else {
        return Ok(Authorization::new(Principal::anonymous()));
    };
    let member = Member::load_from_repo(repo, member_id)
        .with_context(|| format!("cannot validate identity `{member_id}` against repository members"))?
        .with_context(|| {
            format!(
                "unauthorized: unknown member `{member_id}`; use `git forge member ls` to choose a repository member"
            )
        })?;
    let authorization = Authorization::new(Principal::member(&member));
    Ok(if member.role == "maintainer" {
        authorization.administrator()
    } else {
        authorization
    })
}

fn run_issue(
    repo: &gix::Repository,
    authorization: &Authorization,
    command: IssueCommand,
) -> Result<()> {
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
                reporters: Vec::new(),
                edit: None,
            };
            println!("{}", issue.create_in_repo_as(repo, authorization)?);
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
            println!("{}", issue.save_in_repo_as(repo, authorization)?);
        }
        IssueCommand::Show { id } => {
            let id = resolve_issue_show_id(repo, &id)?.with_context(|| format!("no issue {id}"))?;
            let issue =
                Issue::load_from_repo(repo, &id)?.with_context(|| format!("no issue {id}"))?;
            print_issue(&issue);
        }
        IssueCommand::List => {
            let ids = Issue::list(repo)?;
            print_issue_list(repo, &ids)?;
        }
        IssueCommand::Log { id } => print_log("issue", &id, repo, Issue::history(repo, &id)?)?,
        IssueCommand::Rm { id } => {
            if !Issue::delete_as(repo, &id, authorization)? {
                bail!("no issue {id}");
            }
        }
        IssueCommand::Search(args) => {
            let ids =
                gix_forge::search_issue(repo, args.assignee.as_deref(), args.keyword.as_deref())?;
            print_issue_list(repo, &ids)?;
        }
        IssueCommand::Query(args) => run_query_args(repo, args)?,
    }
    Ok(())
}

fn run_member(
    repo: &gix::Repository,
    authorization: &Authorization,
    as_member: Option<&str>,
    command: MemberCommand,
) -> Result<()> {
    match command {
        MemberCommand::Add(args) => {
            let members = load_members(repo)?;
            if args.bootstrap && !members.is_empty() {
                bail!("--bootstrap is only allowed when no members exist");
            }
            if args.bootstrap && args.role.is_some() {
                bail!("--bootstrap forces the role to maintainer; omit --role");
            }

            let explicit_signing_key = args.signing_key.is_some();
            let configured_signing_key = effective_signing_key()?;
            let interactive = resolve_interactive(
                args.interactive,
                args.signing_key.is_none() && args.role.is_none(),
            )?;
            let signing_key = if let Some(signing_key) = args.signing_key {
                signing_key
            } else if interactive {
                match configured_signing_key.as_deref() {
                    Some(signing_key) => prompt_text_with_default("Signing key", signing_key)?,
                    None => prompt_required_text("Signing key")?,
                }
            } else {
                configured_signing_key
                    .as_ref()
                    .context(
                        "cannot determine signing key; set it with `git config user.signingKey ...` or pass `--signing-key KEY`",
                    )?
                    .to_owned()
            };
            let role = if args.bootstrap {
                "maintainer".to_owned()
            } else if interactive {
                prompt_text_with_default("Role", args.role.as_deref().unwrap_or("member"))?
            } else {
                args.role.unwrap_or_else(|| "member".to_owned())
            };
            validate_member_role(&role)?;
            if let Some(existing) = members
                .iter()
                .find(|member| member.signing_key == signing_key)
            {
                bail!(
                    "signing key already belongs to member {}; choose a different key",
                    existing.id
                );
            }

            let authorization = if args.bootstrap {
                Authorization::new(Principal::member_id("bootstrap")).administrator()
            } else if as_member.is_some() {
                authorization.clone()
            } else {
                let matches: Vec<_> = members
                    .iter()
                    .filter(|member| {
                        configured_signing_key.as_deref() == Some(member.signing_key.as_str())
                    })
                    .collect();
                if !explicit_signing_key && configured_signing_key.is_none() {
                    bail!(
                        "cannot infer the authorizing member from user.signingKey; pass `--as MEMBER_ID`"
                    );
                }
                match matches.as_slice() {
                    [member] => resolve_authorization(repo, Some(&member.id))?,
                    [] => bail!(
                        "cannot infer the authorizing member from user.signingKey; pass `--as MEMBER_ID`"
                    ),
                    _ => bail!("user.signingKey matches multiple members; pass `--as MEMBER_ID`"),
                }
            };
            let member = Member {
                id: String::new(),
                signing_key,
                role,
            };
            println!("{}", member.create_in_repo_as(repo, &authorization)?);
        }
        MemberCommand::Edit(args) => {
            let id = resolve_member_show_id(repo, &args.id)?
                .with_context(|| format!("no member {}", args.id))?;
            let mut member =
                Member::load_from_repo(repo, &id)?.with_context(|| format!("no member {id}"))?;
            let no_args_supplied = args.signing_key.is_none() && args.role.is_none();
            let interactive = resolve_interactive(args.interactive, no_args_supplied)?;
            if interactive {
                member.signing_key = prompt_text_with_default("Signing key", &member.signing_key)?;
                member.role = prompt_text_with_default("Role", &member.role)?;
            } else {
                if no_args_supplied {
                    bail!("--signing-key or --role is required unless running interactively");
                }
                if let Some(signing_key) = args.signing_key {
                    member.signing_key = signing_key;
                }
                if let Some(role) = args.role {
                    member.role = role;
                }
            }
            println!("{}", member.save_in_repo_as(repo, authorization)?);
        }
        MemberCommand::Show { id } => {
            let id =
                resolve_member_show_id(repo, &id)?.with_context(|| format!("no member {id}"))?;
            let member =
                Member::load_from_repo(repo, &id)?.with_context(|| format!("no member {id}"))?;
            print_member(&member);
        }
        MemberCommand::List => print_member_list(repo)?,
        MemberCommand::Log { id } => print_log("member", &id, repo, Member::history(repo, &id)?)?,
        MemberCommand::Rm { id } => {
            if !Member::delete_as(repo, &id, authorization)? {
                bail!("no member {id}");
            }
        }
    }
    Ok(())
}

fn run_comment(
    repo: &gix::Repository,
    authorization: &Authorization,
    command: CommentCommand,
) -> Result<()> {
    match command {
        CommentCommand::Add(args) => {
            let interactive = resolve_interactive(
                args.interactive,
                args.on.is_none()
                    && args.subject.is_none()
                    && args.author.is_none()
                    && args.body.is_none(),
            )?;
            let (kind, subject_id, body) = if interactive {
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
                    args.body
                        .context("--body is required unless running interactively")?,
                )
            };
            let id =
                Comment::create_under_as(repo, authorization, &kind, &subject_id, &body, None)?;
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
            println!("{}", comment.save_in_repo_as(repo, authorization)?);
        }
        CommentCommand::Show { id } => {
            let id =
                resolve_comment_show_id(repo, &id)?.with_context(|| format!("no comment {id}"))?;
            let comment =
                Comment::load_from_repo(repo, &id)?.with_context(|| format!("no comment {id}"))?;
            print_comment(&comment);
        }
        CommentCommand::List => {
            let ids = Comment::list(repo)?;
            print_comment_list(repo, &ids)?;
        }
        CommentCommand::Log { id } => {
            print_log("comment", &id, repo, Comment::history(repo, &id)?)?;
        }
        CommentCommand::Rm { id } => {
            if !Comment::delete_as(repo, &id, authorization)? {
                bail!("no comment {id}");
            }
        }
        CommentCommand::Search(args) => {
            let ids =
                gix_forge::search_comment(repo, args.author.as_deref(), args.keyword.as_deref())?;
            print_comment_list(repo, &ids)?;
        }
        CommentCommand::Query(args) => run_query_args(repo, args)?,
    }
    Ok(())
}

fn resolve_member_show_id(repo: &gix::Repository, id: &str) -> Result<Option<String>> {
    resolve_show_id("member", id, &Member::list(repo)?)
}

fn load_members(repo: &gix::Repository) -> Result<Vec<Member>> {
    Member::list(repo)?
        .into_iter()
        .map(|id| Member::load_from_repo(repo, &id)?.with_context(|| format!("no member {id}")))
        .collect()
}

fn resolve_ui_member_id(repo: &gix::Repository) -> Option<String> {
    let signing_key = effective_signing_key().ok().flatten()?;
    let members = load_members(repo).ok()?;
    let mut matches = members
        .into_iter()
        .filter(|member| member.signing_key == signing_key);
    let member = matches.next()?;
    matches.next().is_none().then_some(member.id)
}

fn effective_signing_key() -> Result<Option<String>> {
    let output = std::process::Command::new("git")
        .args(["config", "--get", "user.signingKey"])
        .output()
        .context("failed to read effective Git config user.signingKey")?;
    if !output.status.success() {
        return Ok(None);
    }
    let signing_key = String::from_utf8(output.stdout)
        .context("effective Git config user.signingKey is not valid UTF-8")?;
    Ok((!signing_key.trim().is_empty()).then(|| signing_key.trim().to_owned()))
}

fn validate_member_role(role: &str) -> Result<()> {
    if matches!(role, "member" | "reviewer" | "maintainer") {
        return Ok(());
    }
    bail!("invalid member role `{role}`; choose member, reviewer, or maintainer")
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

fn prompt_comment_fields(repo: &gix::Repository) -> Result<(String, String, String)> {
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
    let body = prompt_body("")?;
    Ok((kind, subject_id, body))
}

fn run_install(
    repo: &gix::Repository,
    authorization: &Authorization,
    args: InstallArgs,
) -> Result<()> {
    let mut installed = false;

    if should_install(args.interactive, "issue schema")? {
        println!(
            "{} {}",
            Issue::KIND,
            gix_forge::ensure_issue_schema_as(repo, authorization)?
        );
        installed = true;
    }

    if should_install(args.interactive, "review schema")? {
        println!(
            "{} {}",
            Review::KIND,
            gix_forge::ensure_review_schema_as(repo, authorization)?
        );
        installed = true;
    }

    if should_install(args.interactive, "member schema")? {
        println!(
            "{} {}",
            Member::KIND,
            gix_forge::ensure_member_schema_as(repo, authorization)?
        );
        installed = true;
    }

    if should_install(args.interactive, "query rules")? {
        gix_forge::install_builtin_query_rules_as(repo, authorization)?;
        println!("query rules review");
        installed = true;
    }

    if !installed {
        bail!("no schemas selected for installation");
    }

    Ok(())
}

fn run_uninstall(repo: &gix::Repository, args: UninstallArgs) -> Result<()> {
    if should_uninstall(args.interactive)? {
        gix_forge::uninstall(repo)?;
        println!("forge uninstalled");
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

fn should_uninstall(interactive: bool) -> Result<bool> {
    if !interactive {
        return Ok(true);
    }
    require_terminal_for_interactive()?;

    Ok(Confirm::with_theme(&interactive_theme())
        .with_prompt("uninstall forge schemas and query rules?")
        .default(false)
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
    let mut theme = ColorfulTheme::default();
    theme.unchecked_item_prefix = theme.unpicked_item_prefix.clone();
    theme
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

struct ValueCompletion<'a> {
    known: &'a [String],
}

impl Completion for ValueCompletion<'_> {
    fn get(&self, input: &str) -> Option<String> {
        let prefix_end = input.rfind(',').map_or(0, |index| index + 1);
        let prefix = &input[..prefix_end];
        let value = input[prefix_end..].trim();
        self.known
            .iter()
            .find(|candidate| candidate.starts_with(value) && candidate != &value)
            .map(|candidate| format!("{prefix}{candidate}"))
    }
}

fn prompt_multi_values(label: &str, known: &[String]) -> Result<Vec<String>> {
    require_terminal_for_interactive()?;
    let completion = ValueCompletion { known };
    let value = Input::<String>::with_theme(&interactive_theme())
        .with_prompt(format!("{label} (comma-separated, optional)"))
        .allow_empty(true)
        .completion_with(&completion)
        .interact_text()?;
    Ok(parse_csv_input(&value))
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
}

fn prompt_issue_fields(repo: &gix::Repository) -> Result<IssueFields> {
    let title = prompt_optional_text("Title")?;
    let body = prompt_body("")?;
    let labels = prompt_multi_values("Labels", &known_issue_values(repo, |issue| &issue.labels)?)?;
    let assignees = prompt_multi_values(
        "Assignees",
        &known_issue_values(repo, |issue| &issue.assignees)?,
    )?;
    Ok(IssueFields {
        title,
        body,
        labels,
        assignees,
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

fn prompt_review_fields(repo: &gix::Repository) -> Result<(String, Vec<String>, String)> {
    let body = prompt_body("")?;
    let reviewers = prompt_multi_values(
        "Reviewers",
        &known_review_values(repo, |review| &review.reviewers)?,
    )?;
    let target = prompt_required_text("Target (e.g. commit:<oid>, blob:<path>:<oid>)")?;
    Ok((body, reviewers, target))
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
            print_issue_list(repo, &ids)?;
        }
        QueryCommand::Reviewer { name } => {
            let ids = gix_forge::search_reviewer(repo, &name)?;
            print_review_list(repo, &ids)?;
        }
        QueryCommand::Requester { name } => {
            let ids = gix_forge::search_requester(repo, &name)?;
            print_review_list(repo, &ids)?;
        }
        QueryCommand::Keyword { value } => {
            let hits = gix_forge::search_keyword(repo, &value)?;
            print_search_views(repo, &hits)?;
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
            print_search_views(repo, &hits)?;
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

fn run_review(
    repo: &gix::Repository,
    authorization: &Authorization,
    command: ReviewCommand,
) -> Result<()> {
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
            let (body, reviewers, target) = if interactive {
                prompt_review_fields(repo)?
            } else {
                (
                    args.body
                        .context("--body is required unless running interactively")?,
                    args.reviewers,
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
                requesters: Vec::new(),
                target: ReviewTarget::parse(&target)?,
                edit: None,
            };
            println!("{}", review.create_in_repo_as(repo, authorization)?);
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
            println!("{}", review.save_in_repo_as(repo, authorization)?);
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
            print_review_list(repo, &ids)?;
        }
        ReviewCommand::Log { id } => print_log("review", &id, repo, Review::history(repo, &id)?)?,
        ReviewCommand::Rm { id } => {
            if !Review::delete_as(repo, &id, authorization)? {
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
            print_review_list(repo, &ids)?;
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
    let display_id = shorten_entity_id(id);
    let min_len = min_unique_prefix_len(id, ids).min(display_id.len());
    let (prefix, suffix) = display_id.split_at(min_len);
    format!("**{prefix}**{suffix}")
}

fn shorten_entity_id(id: &str) -> &str {
    let id = id.rsplit('/').next().unwrap_or(id);
    id.get(..8).unwrap_or(id)
}

fn display_entity_id(id: &str) -> String {
    format!("#{}", shorten_entity_id(id))
}

fn display_entity_reference(reference: &str) -> String {
    let Some((kind, id)) = reference.split_once(':') else {
        return reference.to_owned();
    };
    format!("{kind}:{}", shorten_entity_id(id))
}

fn display_comment_subject(subject: Option<&str>) -> String {
    subject.map_or_else(|| "(none)".to_owned(), display_entity_reference)
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

fn print_search_views(repo: &gix::Repository, hits: &[gix_forge::SearchHit]) -> Result<()> {
    let issue_ids: Vec<String> = hits
        .iter()
        .filter(|hit| hit.kind == HitKind::Issue)
        .map(|hit| hit.id.clone())
        .collect();
    let review_ids: Vec<String> = hits
        .iter()
        .filter(|hit| hit.kind == HitKind::Review)
        .map(|hit| hit.id.clone())
        .collect();
    if issue_ids.is_empty() && review_ids.is_empty() {
        println!("No matches");
        return Ok(());
    }
    if !issue_ids.is_empty() {
        print_issue_list(repo, &issue_ids)?;
    }
    if !review_ids.is_empty() {
        print_review_list(repo, &review_ids)?;
    }
    Ok(())
}

/// Every entity's own `list` command loads each id and formats a table row
/// the same way -- `to_row` supplies only what's genuinely specific to the
/// kind (its two non-status columns, and its status if it has one).
fn entity_rows<T: EntityOps>(
    repo: &gix::Repository,
    ids: &[String],
    to_row: impl Fn(&T) -> (String, String, String),
) -> Result<Vec<Vec<String>>> {
    let mut rows = Vec::new();
    for id in ids {
        let Some(entity) = T::load_from_repo(repo, id)? else {
            continue;
        };
        let (col2, col3, status) = to_row(&entity);
        if matches!(Status::parse(&status), Some(Status::Closed)) {
            continue;
        }
        rows.push(vec![
            display_entity_id(id),
            col2,
            col3,
            status,
            updated_relative(repo, T::history(repo, id)?)?,
        ]);
    }
    Ok(rows)
}

fn print_issue_list(repo: &gix::Repository, ids: &[String]) -> Result<()> {
    let rows = entity_rows::<Issue>(repo, ids, |issue| {
        (
            title_or_untitled(&issue.title).to_owned(),
            join_values_or_none(&issue.labels),
            issue.status.clone(),
        )
    })?;
    if rows.is_empty() && !ids.is_empty() {
        println!("No unresolved issues");
        return Ok(());
    }
    print_entity_table("issues", "TITLE", "LABELS", rows);
    Ok(())
}

fn print_review_list(repo: &gix::Repository, ids: &[String]) -> Result<()> {
    let rows = entity_rows::<Review>(repo, ids, |review| {
        (
            review.target.to_string(),
            join_values_or_none(&review.reviewers),
            review.status.clone(),
        )
    })?;
    if rows.is_empty() && !ids.is_empty() {
        println!("No unresolved reviews");
        return Ok(());
    }
    print_entity_table("reviews", "TARGET", "REVIEWERS", rows);
    Ok(())
}

fn print_member_list(repo: &gix::Repository) -> Result<()> {
    let ids = Member::list(repo)?;
    let rows = entity_rows::<Member>(repo, &ids, |member| {
        (
            member.signing_key.clone(),
            member.role.clone(),
            "-".to_owned(),
        )
    })?;
    print_entity_table("members", "SIGNING KEY", "ROLE", rows);
    Ok(())
}

fn print_comment_list(repo: &gix::Repository, ids: &[String]) -> Result<()> {
    let rows = entity_rows::<Comment>(repo, ids, |comment| {
        (
            display_comment_subject(comment.subject.as_deref()),
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
        color_id(&display_entity_id(id))
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

fn print_member(member: &Member) {
    print_show_doc(ShowDoc {
        kind: "member",
        id: &member.id,
        title: None,
        meta: vec![
            format!("signing key: {}", member.signing_key),
            format!("role: {}", member.role),
        ],
        body: "",
    });
}

fn print_comment(comment: &Comment) {
    let mut meta = vec![
        format!(
            "subject: {}",
            display_comment_subject(comment.subject.as_deref())
        ),
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
            display_entity_id(doc.id).yellow()
        );
    } else {
        println!("{} {}", doc.kind.bold(), display_entity_id(doc.id).yellow());
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
