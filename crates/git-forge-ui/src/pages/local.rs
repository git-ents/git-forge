//! The "Local" ambient strip: cheap repository-local status and gate state.

use std::{collections::HashSet, process::Command};

use gix::bstr::BString;
use gix_forge::GateResult;
use topcoat::{
    Result,
    context::Cx,
    router::{content::Form, page},
    view::view,
};

use crate::{auth::authorization, confirm::confirm_step, pages::with_repo, shell::shell};

const ENGINE_VERSION: &str = env!("CARGO_PKG_VERSION");

struct LocalState {
    head: String,
    dirty_files: usize,
    unpushed_refs: usize,
    gates: Vec<GateResult>,
}

fn local_state(repo: &gix::Repository) -> std::result::Result<LocalState, gix_forge::Error> {
    let head = repo
        .head_id()
        .map(|id| id.to_string()[..7].to_owned())
        .unwrap_or_else(|_| "unborn".to_owned());

    let status = repo
        .status(gix::progress::Discard)
        .map_err(|error| gix_forge::Error::QueryRules(error.to_string()))?;
    let mut dirty_paths = HashSet::new();
    for item in status
        .into_iter(std::iter::empty::<BString>())
        .map_err(|error| gix_forge::Error::QueryRules(error.to_string()))?
    {
        let item = item.map_err(|error| gix_forge::Error::QueryRules(error.to_string()))?;
        dirty_paths.insert(item.location().to_string());
    }

    let mut forge_refs = Vec::new();
    let reference_store = repo
        .references()
        .map_err(|error| gix_forge::Error::QueryRules(error.to_string()))?;
    let mut references = reference_store
        .all()
        .map_err(|error| gix_forge::Error::QueryRules(error.to_string()))?;
    while let Some(reference) = references
        .next()
        .transpose()
        .map_err(|error| gix_forge::Error::QueryRules(error.to_string()))?
    {
        let name = reference.name().to_string();
        if name.starts_with("refs/forge/") {
            forge_refs.push(name);
        }
    }

    // Remote-tracking refs are the cheap local approximation of what the
    // configured remote already has. A fetch refreshes this view; pushing does
    // not require walking every forge commit.
    let remote = repo
        .remote_names()
        .iter()
        .next()
        .map(|name| name.to_string());
    let unpushed_refs = forge_refs
        .iter()
        .filter(|name| {
            remote.as_ref().is_none_or(|remote| {
                !repo
                    .try_find_reference(
                        format!("refs/remotes/{remote}/{}", name.trim_start_matches("refs/"))
                            .as_str(),
                    )
                    .ok()
                    .flatten()
                    .is_some()
            })
        })
        .count();

    let gates = gix_forge::evaluate_gates(repo).unwrap_or_default();
    Ok(LocalState {
        head,
        dirty_files: dirty_paths.len(),
        unpushed_refs,
        gates,
    })
}

async fn load_local_state(cx: &Cx) -> LocalState {
    with_repo(cx, local_state).await.unwrap_or(LocalState {
        head: "unknown".to_owned(),
        dirty_files: 0,
        unpushed_refs: 0,
        gates: Vec::new(),
    })
}

pub(crate) async fn local_strip(cx: &Cx) -> Result {
    let state = load_local_state(cx).await;
    let gate_failure = state.gates.iter().any(|gate| !gate.passed);
    let gate_text = if state.gates.is_empty() {
        "gates unavailable (install the built-in rules)".to_owned()
    } else if gate_failure {
        "one or more merge/release gates fail for the current repository state".to_owned()
    } else {
        "all evaluated gates pass for the current repository state".to_owned()
    };
    let gate_class = if state.gates.is_empty() {
        "warn"
    } else if gate_failure {
        "danger"
    } else {
        "ok"
    };
    view! { cx =>
        <div class="local-strip">
            <div id="local" class="local-panel">
                <p class="local-panel-heading">"Local — what your uncommitted work does to the record"</p>
                <div class="local-panel-body">
                    <p>(state.dirty_files) " dirty files at HEAD " <code class="oid">(state.head.as_str())</code></p>
                    <p>(state.unpushed_refs) " forge refs are not present in the configured remote"</p>
                    <p class=(gate_class)>(gate_text.as_str())</p>
                </div>
                <form class="local-push-form" action="/local/push" method="post">
                    <button type="submit" disabled=(state.unpushed_refs == 0)>"Push forge refs"</button>
                </form>
                <a class="local-panel-close" href="#">"Close"</a>
            </div>
            <div class="local-bar">
                <a class="local-bar-item" href="#local"><span class="status-dot ok"></span>"HEAD " <code class="oid">(state.head.as_str())</code></a>
                <span class="local-bar-item warn">(state.dirty_files) " dirty"</span>
                <a class="local-bar-item warn local-bar-link" href="#local">(state.unpushed_refs) " forge refs unpushed"</a>
                <span class="local-bar-item local-bar-engine">"engine v" (ENGINE_VERSION)</span>
            </div>
        </div>
    }
}

#[page(POST "/local/push")]
async fn local_push(
    cx: &Cx,
    Form(input): Form<std::collections::HashMap<String, String>>,
) -> Result {
    let _authorization = authorization(cx).await?;
    let preview = with_repo(cx, |repo| {
        let refs = forge_ref_names(repo)?;
        Ok((repo.workdir().map(|path| path.to_owned()), refs))
    })
    .await
    .map_err(|error| anyhow::anyhow!(error))?;
    let (work_dir, refs) = preview;
    let summary = (view! { cx =>
        "Push " (refs.len()) " forge ref(s) from this repository to its configured remote."
    })?;
    if let Some(confirm) = confirm_step(
        cx,
        &input,
        "/local/push",
        "refs/forge/**",
        "/inbox",
        summary,
    )
    .await?
    {
        return view! { shell(active: None, title: "Confirm push", keyword: None, child: confirm) };
    }

    let result: std::result::Result<(), String> = match (work_dir, refs.first()) {
        (Some(work_dir), Some(_)) => {
            with_repo(cx, move |repo| push_forge_refs(repo, &work_dir)).await
        }
        _ => Err("no forge refs or worktree to push".to_owned()),
    };
    let content = match result {
        Ok(()) => view! { cx =>
            <section class="success-panel"><span class="success-icon">"✓"</span><p class="eyebrow">"PUBLISHED"</p><h2>"Forge refs pushed"</h2><p class="muted">"The configured remote accepted the forge ref update."</p><a class="button-link" href="/inbox">"Back to Inbox"</a></section>
        }?,
        Err(error) => view! { cx =>
            <section class="error-panel"><h2>"Push failed"</h2><p>(error)</p><a class="button-link secondary" href="/inbox">"Back to Inbox"</a></section>
        }?,
    };
    view! { shell(active: None, title: "Push forge refs", keyword: None, child: content) }
}

fn forge_ref_names(repo: &gix::Repository) -> std::result::Result<Vec<String>, gix_forge::Error> {
    let mut refs = Vec::new();
    let references = repo
        .references()
        .map_err(|e| gix_forge::Error::QueryRules(e.to_string()))?;
    let mut all = references
        .all()
        .map_err(|e| gix_forge::Error::QueryRules(e.to_string()))?;
    while let Some(reference) = all
        .next()
        .transpose()
        .map_err(|e| gix_forge::Error::QueryRules(e.to_string()))?
    {
        let name = reference.name().to_string();
        if name.starts_with("refs/forge/") {
            refs.push(name);
        }
    }
    Ok(refs)
}

fn push_forge_refs(
    repo: &gix::Repository,
    work_dir: &std::path::Path,
) -> std::result::Result<(), gix_forge::Error> {
    let remote = repo
        .remote_names()
        .iter()
        .next()
        .map(|name| name.to_string())
        .ok_or_else(|| gix_forge::Error::QueryRules("no configured remote".to_owned()))?;
    let refs = forge_ref_names(repo)?;
    let mut command = Command::new("git");
    command
        .current_dir(work_dir)
        .args(["push", remote.as_str()]);
    for reference in refs {
        command.arg(format!("{reference}:{reference}"));
    }
    let output = command
        .output()
        .map_err(|error| gix_forge::Error::QueryRules(error.to_string()))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(gix_forge::Error::QueryRules(
            String::from_utf8_lossy(&output.stderr).into_owned(),
        ))
    }
}
