use std::collections::HashMap;

use gix_forge::{Comment, Commentable, EntityOps, Issue, Member, Status};
use topcoat::{
    Result,
    context::Cx,
    router::{content::Form, page, path_param},
    view::{Unescaped, view},
};

use crate::{
    auth::{authorization, authorization_error, can_create, can_update},
    confirm::confirm_step,
    pages::{csv_values, with_repo},
    render::render_asciidoc,
    shell::{Tab, route_shell, shell},
};

#[path_param(error = bad_request)]
struct IssueId(String);

#[derive(Clone)]
struct IssueForm {
    title: String,
    body: String,
    status: String,
    labels: String,
    assignees: String,
    reporters: String,
}

impl IssueForm {
    fn from_issue(issue: &Issue) -> Self {
        Self {
            title: issue.title.clone(),
            body: issue.body.clone(),
            status: issue.status.clone(),
            labels: issue.labels.join(", "),
            assignees: issue.assignees.join(", "),
            reporters: issue.reporters.join(", "),
        }
    }

    fn from_input(input: &HashMap<String, String>) -> Self {
        Self {
            title: input.get("title").cloned().unwrap_or_default(),
            body: input.get("body").cloned().unwrap_or_default(),
            status: input
                .get("status")
                .cloned()
                .unwrap_or_else(|| "open".to_owned()),
            labels: input.get("labels").cloned().unwrap_or_default(),
            assignees: input.get("assignees").cloned().unwrap_or_default(),
            reporters: input.get("reporters").cloned().unwrap_or_default(),
        }
    }
}

async fn issue_form(cx: &Cx, action: &str, form: &IssueForm, error: Option<&str>) -> Result {
    view! { cx =>
        <section class="form-layout">
            <div class="form-intro">
                <p class="eyebrow">"ISSUE TRACKING"</p>
                <h2>"Tell the team what happened"</h2>
                <p class="muted">"Keep the title crisp, then add the context people need to act."</p>
            </div>
            if let Some(error) = error {
                <div class="error-panel"><strong>"Issue could not be saved"</strong><p>(error)</p></div>
            }
            <form class="entity-form" action=(action) method="post">
                <fieldset>
                    <legend>"Issue details"</legend>
                    <label for="issue-title">"Title"</label>
                    <input id="issue-title" name="title" value=(form.title.as_str()) placeholder="A clear, concise summary" required="">
                    <label for="issue-body">"Description " <span class="muted">"(optional)"</span></label>
                    <textarea id="issue-body" name="body" rows="10" placeholder="What should someone know to reproduce or resolve this?">(form.body.as_str())</textarea>
                </fieldset>
                <fieldset>
                    <legend>"Triage"</legend>
                    <label for="issue-status">"Status"</label>
                    <select id="issue-status" name="status">
                        if form.status == "closed" {
                            <option value="open">"Open"</option>
                            <option value="closed" selected="">"Closed"</option>
                        } else {
                            <option value="open" selected="">"Open"</option>
                            <option value="closed">"Closed"</option>
                        }
                    </select>
                    <label for="issue-labels">"Labels " <span class="muted">"(comma-separated)"</span></label>
                    <input id="issue-labels" name="labels" value=(form.labels.as_str()) placeholder="bug, documentation">
                    <label for="issue-assignees">"Assignees " <span class="muted">"(comma-separated)"</span></label>
                    <input id="issue-assignees" name="assignees" value=(form.assignees.as_str()) placeholder="handle or member ID">
                    <label for="issue-reporters">"Reporters " <span class="muted">"(comma-separated)"</span></label>
                    <input id="issue-reporters" name="reporters" value=(form.reporters.as_str()) placeholder="handle or member ID">
                </fieldset>
                <div class="form-actions">
                    <a class="button-link secondary" href="/issues">"Cancel"</a>
                    <button type="submit">"Save issue"</button>
                </div>
            </form>
        </section>
    }
}

/// One master-list row's worth of real data: the issue plus its comment
/// count (there is no cheaper aggregate for this than loading the thread,
/// so callers pay for it once per issue when building the list).
type IssueRow = (Issue, usize);

async fn issue_list(cx: &Cx, items: &[IssueRow], selected: Option<&str>) -> Result {
    let can_create = can_create::<Issue>(cx).await;
    view! { cx =>
        <div class="list-heading">
            <div><h2>"Issues"</h2><span class="muted">(items.len())</span></div>
            if can_create {
                <a class="button-link" href="/issues/new">"New issue"</a>
            }
        </div>
        if items.is_empty() {
            <div class="empty-panel">
                <p>"No issues found."</p>
                if can_create {
                    <a class="button-link" href="/issues/new">"Open the first issue"</a>
                } else {
                    <p class="form-help">"Authenticate as a forge member to open the first issue."</p>
                }
            </div>
        } else {
            <ul class="entity-list">
                for (issue, comment_count) in items {
                    let dot_class = if issue.status == "open" { "status-dot ok" } else { "status-dot" };
                    let row_class = if selected == Some(issue.id.as_str()) { "list-row selected" } else { "list-row" };
                    let author = issue.reporters.first().map(String::as_str).unwrap_or("unassigned");
                    <li>
                        <a class=(row_class) href=(format!("/issues/{}", issue.id))>
                            <div class="row-top">
                                <span class=(dot_class)></span>
                                <strong class="row-title">(issue.title.as_str())</strong>
                            </div>
                            <div class="row-meta">
                                <span class="row-id">(issue.id.as_str())</span>
                                <span>(author)</span>
                                <span class="row-comments">(*comment_count) " comments"</span>
                                for label in &issue.labels {
                                    <span class="label-chip">(label.as_str())</span>
                                }
                            </div>
                        </a>
                    </li>
                }
            </ul>
        }
    }
}

/// Attach comment counts the same way the detail view needs them, for the
/// list pane both routes share.
fn load_issue_rows(repo: &gix::Repository) -> std::result::Result<Vec<IssueRow>, gix_forge::Error> {
    let ids = Issue::list(repo)?;
    ids.into_iter()
        .map(|id| Issue::load_from_repo(repo, &id))
        .collect::<std::result::Result<Vec<_>, _>>()
        .map(|loaded| {
            loaded
                .into_iter()
                .flatten()
                .map(|issue| {
                    let comment_count = issue.get_comments(repo).map(|c| c.len()).unwrap_or(0);
                    (issue, comment_count)
                })
                .collect()
        })
}

/// The signed-in member's registered signing key for `author`, when they are
/// still a known member -- the one piece of "signed" identity a comment
/// genuinely carries, since [`Comment`] itself stores no per-comment
/// signature or timestamp.
fn signing_key_of(repo: &gix::Repository, author: &str) -> Option<String> {
    let member = Member::load_from_repo(repo, author).ok().flatten()?;
    let key = member.signing_key.trim();
    (!key.is_empty()).then(|| key.to_owned())
}

#[page("/issues")]
async fn issues(cx: &Cx) -> Result {
    let data = with_repo(cx, load_issue_rows).await;

    let (list, detail) = match data {
        Ok(issues) => (
            issue_list(cx, &issues, None).await?,
            (view! {
                <div class="placeholder-detail">
                    <p class="eyebrow">"ISSUE DETAIL"</p>
                    <h2>"Select an issue"</h2>
                    <p class="muted">"Choose an issue from the list to read its body and comments, or open a new one."</p>
                </div>
            })?,
        ),
        Err(error) => (
            (view! { <p class="error-text">(error)</p> })?,
            (view! { <p class="error-text">"Could not load issues."</p> })?,
        ),
    };

    view! { route_shell(active: Tab::Issues, title: "Issues", nav_width: "21rem", nav: list, detail: detail) }
}

#[page("/issues/new")]
async fn issue_new(cx: &Cx) -> Result {
    if !can_create::<Issue>(cx).await {
        let content = (view! { <div class="empty-panel"><h2>"Sign in to open an issue"</h2><p class="muted">"Reads stay available without an account; creating an issue requires a forge member identity."</p><a class="button-link secondary" href="/issues">"Back to issues"</a></div> })?;
        return view! { shell(active: Some(Tab::Issues), title: "New issue", keyword: None, child: content) };
    }
    let form = IssueForm {
        title: String::new(),
        body: String::new(),
        status: "open".to_owned(),
        labels: String::new(),
        assignees: String::new(),
        reporters: String::new(),
    };
    let content = issue_form(cx, "/issues", &form, None).await?;
    view! { shell(active: Some(Tab::Issues), title: "New issue", keyword: None, child: content) }
}

#[page(POST "/issues")]
async fn issue_create(cx: &Cx, Form(input): Form<HashMap<String, String>>) -> Result {
    let authorization = authorization(cx).await?;
    let form = IssueForm::from_input(&input);
    let title = form.title.trim();
    if title.is_empty() {
        let content = issue_form(cx, "/issues", &form, Some("A title is required.")).await?;
        return view! { shell(active: Some(Tab::Issues), title: "New issue", keyword: None, child: content) };
    }
    if Status::parse(&form.status).is_none() {
        let content = issue_form(
            cx,
            "/issues",
            &form,
            Some("Choose either Open or Closed for the status."),
        )
        .await?;
        return view! { shell(active: Some(Tab::Issues), title: "New issue", keyword: None, child: content) };
    }

    let issue = Issue {
        id: String::new(),
        status: form.status.trim().to_ascii_lowercase(),
        title: title.to_owned(),
        body: form.body.trim().to_owned(),
        labels: csv_values(&form.labels),
        assignees: csv_values(&form.assignees),
        reporters: csv_values(&form.reporters),
        edit: None,
    };
    let result = with_repo(cx, move |repo| {
        issue.create_in_repo_as(repo, &authorization)
    })
    .await;
    match result {
        Ok(id) => {
            let content = (view! {
                <section class="success-panel">
                    <span class="success-icon">"✓"</span>
                    <p class="eyebrow">"ISSUE CREATED"</p>
                    <h2>"Your issue is ready for triage"</h2>
                    <p class="muted">"The new issue was written to the repository."</p>
                    <a class="button-link" href=(format!("/issues/{id}"))>"View issue"</a>
                </section>
            })?;
            view! { shell(active: Some(Tab::Issues), title: "Issue created", keyword: None, child: content) }
        }
        Err(error) => {
            if let Some(error) = authorization_error(&error) {
                return Err(error);
            }
            let content = issue_form(cx, "/issues", &form, Some(&error)).await?;
            view! { shell(active: Some(Tab::Issues), title: "New issue", keyword: None, child: content) }
        }
    }
}

#[page("/issues/{issue_id}")]
async fn issue_detail(cx: &Cx) -> Result {
    let id = path_param::<IssueId>(cx)?.clone();
    let data = with_repo(cx, move |repo| {
        let issue = Issue::load_from_repo(repo, &id)?;
        match issue {
            Some(issue) => {
                let comments = issue.get_comments(repo)?;
                let comment_rows: Vec<(Comment, Option<String>)> = comments
                    .into_iter()
                    .map(|comment| {
                        let key = signing_key_of(repo, &comment.author);
                        (comment, key)
                    })
                    .collect();
                let issue_items = load_issue_rows(repo)?;
                Ok(Some((issue_items, issue, comment_rows)))
            }
            None => Ok(None),
        }
    })
    .await;

    let (list, detail) = match data {
        Ok(Some((issue_items, issue, comment_rows))) => {
            let can_edit = can_update(cx, &issue).await;
            let can_comment = can_create::<Comment>(cx).await;
            let next_status = if issue.status == "open" {
                "closed"
            } else {
                "open"
            };
            let status_class = if issue.status == "open" { "status ok" } else { "status neutral" };
            (
                issue_list(cx, &issue_items, Some(issue.id.as_str())).await?,
                (view! {
                    <article>
                        <div class="detail-meta">
                            <span class=(status_class)>(issue.status.as_str())</span>
                            <span class="muted">"#" (issue.id.as_str())</span>
                        </div>
                        <div class="detail-heading">
                            <div><h2>(issue.title.as_str())</h2><p class="muted">"Issue " (issue.id.as_str())</p></div>
                            <div class="detail-actions">
                                if can_edit {
                                    <a class="button-link secondary" href=(format!("/issues/{}/edit", issue.id))>"Edit"</a>
                                    <form class="inline-form" action=(format!("/issues/{}/status", issue.id)) method="post">
                                        <input type="hidden" name="status" value=(next_status)>
                                        <button type="submit">if next_status == "closed" { "Close issue" } else { "Reopen issue" }</button>
                                    </form>
                                    <form class="inline-form" action=(format!("/issues/{}/delete", issue.id)) method="post">
                                        <button class="danger-button" type="submit">"Delete"</button>
                                    </form>
                                } else {
                                    <span class="action-note">"Only the reporter or a maintainer can change this issue."</span>
                                }
                            </div>
                        </div>
                        <div class="body-copy rendered-file">(Unescaped::new_unchecked(render_asciidoc(&issue.body)))</div>
                        <div class="tag-row">
                            for label in &issue.labels { <span class="label-chip">(label.as_str())</span> }
                        </div>
                        <dl class="facts">
                            <dt>"Assignees"</dt><dd>(if issue.assignees.is_empty() { "None".to_owned() } else { issue.assignees.join(", ") })</dd>
                            <dt>"Reporters"</dt><dd>(if issue.reporters.is_empty() { "None".to_owned() } else { issue.reporters.join(", ") })</dd>
                        </dl>
                        <section class="comments">
                            <div class="panel-heading"><div><h3>"Comments"</h3><span class="muted">(comment_rows.len())</span></div> if can_comment { <a class="button-link secondary" href="/comments/new">"New comment"</a> }</div>
                            if comment_rows.is_empty() {
                                <p class="empty">"No comments yet. Start the conversation below."</p>
                            } else {
                                for (comment, key) in &comment_rows {
                                    <article class="comment thread-entry">
                                        <div class="comment-heading">
                                            <strong>(comment.author.as_str())</strong>
                                            if let Some(key) = key {
                                                <span class="thread-key">"signed " <code>(key.as_str())</code></span>
                                            }
                                            if can_update(cx, comment).await {
                                                <a class="muted" href=(format!("/comments/edit/{}", comment.id))>"Edit"</a>
                                            }
                                        </div>
                                        <p>(comment.body.as_str())</p>
                                    </article>
                                }
                            }
                            if can_comment {
                                <form class="comment-form" action=(format!("/issues/{}/comments", issue.id)) method="post">
                                    <label for="issue-comment-author">"Add a comment"</label>
                                    <input id="issue-comment-author" name="author" value="authenticated member" type="hidden">
                                    <textarea name="body" rows="5" placeholder="Leave a helpful update or question." required=""></textarea>
                                    <button type="submit">"Comment"</button>
                                </form>
                            } else {
                                <p class="form-context">"Sign in as a forge member to join this discussion."</p>
                            }
                        </section>
                    </article>
                })?,
            )
        }
        Ok(None) => (
            (view! { <p class="empty">"Issue not found."</p> })?,
            (view! { <p class="empty">"Issue not found."</p> })?,
        ),
        Err(error) => (
            (view! { <p class="error-text">(error)</p> })?,
            (view! { <p class="error-text">"Could not load issue."</p> })?,
        ),
    };

    view! { route_shell(active: Tab::Issues, title: "Issue", nav_width: "21rem", nav: list, detail: detail) }
}

#[page("/issues/{issue_id}/edit")]
async fn issue_edit_page(cx: &Cx) -> Result {
    let id = path_param::<IssueId>(cx)?.clone();
    let data = with_repo(cx, move |repo| Issue::load_from_repo(repo, &id)).await;
    let content = match data {
        Ok(Some(issue)) => {
            if can_update(cx, &issue).await {
                issue_form(
                    cx,
                    &format!("/issues/{}/edit", issue.id),
                    &IssueForm::from_issue(&issue),
                    None,
                )
                .await?
            } else {
                (view! { <div class="empty-panel"><h2>"This issue is read-only for you"</h2><p class="muted">"Only the reporter or a maintainer can edit or delete it."</p><a class="button-link secondary" href=(format!("/issues/{}", issue.id))>"Back to issue"</a></div> })?
            }
        }
        Ok(None) => {
            (view! { <div class="empty-panel"><h2>"Issue not found"</h2><a href="/issues">"Back to issues"</a></div> })?
        }
        Err(error) => {
            (view! { <div class="error-panel"><strong>"Could not load issue"</strong><p>(error)</p></div> })?
        }
    };
    view! { shell(active: Some(Tab::Issues), title: "Edit issue", keyword: None, child: content) }
}

#[page(POST "/issues/{issue_id}/edit")]
async fn issue_edit_submit(cx: &Cx, Form(input): Form<HashMap<String, String>>) -> Result {
    let authorization = authorization(cx).await?;
    let id = path_param::<IssueId>(cx)?.clone();
    let form = IssueForm::from_input(&input);
    if form.title.trim().is_empty() || Status::parse(&form.status).is_none() {
        let content = issue_form(
            cx,
            &format!("/issues/{id}/edit"),
            &form,
            Some("A title is required and status must be Open or Closed."),
        )
        .await?;
        return view! { shell(active: Some(Tab::Issues), title: "Edit issue", keyword: None, child: content) };
    }
    let lookup_id = id.clone();
    let result = with_repo(cx, move |repo| {
        let mut issue = Issue::load_from_repo(repo, &lookup_id)?.ok_or_else(|| {
            gix_forge::Error::InvalidTarget(format!("issue `{lookup_id}` was not found"))
        })?;
        issue.title = form.title.trim().to_owned();
        issue.body = form.body.trim().to_owned();
        issue.status = form.status.trim().to_ascii_lowercase();
        issue.labels = csv_values(&form.labels);
        issue.assignees = csv_values(&form.assignees);
        issue
            .save_in_repo_as(repo, &authorization)
            .map(|_| issue.id)
    })
    .await;
    match result {
        Ok(id) => {
            let content = (view! { <section class="success-panel"><span class="success-icon">"✓"</span><p class="eyebrow">"SAVED"</p><h2>"Issue updated"</h2><a class="button-link" href=(format!("/issues/{id}"))>"View issue"</a></section> })?;
            view! { shell(active: Some(Tab::Issues), title: "Issue updated", keyword: None, child: content) }
        }
        Err(error) => {
            if let Some(error) = authorization_error(&error) {
                return Err(error);
            }
            let content = (view! { <div class="error-panel"><strong>"Issue could not be saved"</strong><p>(error)</p><a href=(format!("/issues/{id}/edit"))>"Return to editor"</a></div> })?;
            view! { shell(active: Some(Tab::Issues), title: "Edit issue", keyword: None, child: content) }
        }
    }
}

#[page(POST "/issues/{issue_id}/status")]
async fn issue_status(cx: &Cx, Form(input): Form<HashMap<String, String>>) -> Result {
    let authorization = authorization(cx).await?;
    let id = path_param::<IssueId>(cx)?.clone();
    let status = input
        .get("status")
        .map(String::as_str)
        .unwrap_or_default()
        .to_owned();
    if let Some(confirm) = confirm_step(
        cx,
        &input,
        &format!("/issues/{id}/status"),
        &format!("refs/meta/issues/{id}/status"),
        &format!("/issues/{id}"),
        (view! { cx => "Change issue " <code>(id.as_str())</code> " status to " <code>(status.as_str())</code> })?,
    )
    .await?
    {
        return view! { shell(active: Some(Tab::Issues), title: "Confirm status change", keyword: None, child: confirm) };
    }
    let lookup_id = id.clone();
    let result = with_repo(cx, move |repo| {
        let mut issue = Issue::load_from_repo(repo, &lookup_id)?.ok_or_else(|| {
            gix_forge::Error::InvalidTarget(format!("issue `{lookup_id}` was not found"))
        })?;
        if Status::parse(&status).is_none() {
            return Err(gix_forge::Error::InvalidTarget(
                "invalid issue status".to_owned(),
            ));
        }
        issue.status = status;
        issue
            .save_in_repo_as(repo, &authorization)
            .map(|_| issue.id)
    })
    .await;
    match result {
        Ok(id) => {
            let content = (view! { <section class="success-panel"><span class="success-icon">"✓"</span><h2>"Issue status updated"</h2><a class="button-link" href=(format!("/issues/{id}"))>"Return to issue"</a></section> })?;
            view! { shell(active: Some(Tab::Issues), title: "Issue updated", keyword: None, child: content) }
        }
        Err(error) => {
            if let Some(error) = authorization_error(&error) {
                return Err(error);
            }
            let content = (view! { <div class="error-panel"><strong>"Could not update issue"</strong><p>(error)</p><a href=(format!("/issues/{id}"))>"Return to issue"</a></div> })?;
            view! { shell(active: Some(Tab::Issues), title: "Issue action failed", keyword: None, child: content) }
        }
    }
}

#[page(POST "/issues/{issue_id}/comments")]
async fn issue_comment(cx: &Cx, Form(input): Form<HashMap<String, String>>) -> Result {
    let authorization = authorization(cx).await?;
    let id = path_param::<IssueId>(cx)?.clone();
    let author = input
        .get("author")
        .map(String::as_str)
        .unwrap_or_default()
        .trim()
        .to_owned();
    let body = input
        .get("body")
        .map(String::as_str)
        .unwrap_or_default()
        .trim()
        .to_owned();
    if author.is_empty() || body.is_empty() {
        let content = (view! { <div class="error-panel"><strong>"Comment could not be saved"</strong><p>"Your name and a comment are required."</p><a href=(format!("/issues/{id}"))>"Return to issue"</a></div> })?;
        return view! { shell(active: Some(Tab::Issues), title: "Comment failed", keyword: None, child: content) };
    }
    if let Some(confirm) = confirm_step(
        cx,
        &input,
        &format!("/issues/{id}/comments"),
        &format!("refs/meta/comments/issue-{id}"),
        &format!("/issues/{id}"),
        (view! { cx => "Post a new comment on issue " <code>(id.as_str())</code> })?,
    )
    .await?
    {
        return view! { shell(active: Some(Tab::Issues), title: "Confirm comment", keyword: None, child: confirm) };
    }
    let lookup_id = id.clone();
    let result = with_repo(cx, move |repo| {
        let issue = Issue::load_from_repo(repo, &lookup_id)?.ok_or_else(|| {
            gix_forge::Error::InvalidTarget(format!("issue `{lookup_id}` was not found"))
        })?;
        issue.add_comment_as(repo, &authorization, &body)
    })
    .await;
    match result {
        Ok(_) => {
            let content = (view! { <section class="success-panel"><span class="success-icon">"✓"</span><h2>"Comment added"</h2><a class="button-link" href=(format!("/issues/{id}"))>"Return to issue"</a></section> })?;
            view! { shell(active: Some(Tab::Issues), title: "Comment added", keyword: None, child: content) }
        }
        Err(error) => {
            if let Some(error) = authorization_error(&error) {
                return Err(error);
            }
            let content = (view! { <div class="error-panel"><strong>"Comment could not be saved"</strong><p>(error)</p><a href=(format!("/issues/{id}"))>"Return to issue"</a></div> })?;
            view! { shell(active: Some(Tab::Issues), title: "Comment failed", keyword: None, child: content) }
        }
    }
}

#[page(POST "/issues/{issue_id}/delete")]
async fn issue_delete(cx: &Cx) -> Result {
    let authorization = authorization(cx).await?;
    let id = path_param::<IssueId>(cx)?.clone();
    let lookup_id = id.clone();
    let result = with_repo(cx, move |repo| {
        Issue::delete_as(repo, &lookup_id, &authorization)
    })
    .await;
    let content = match result {
        Ok(true) => {
            (view! { <section class="success-panel"><span class="success-icon">"✓"</span><h2>"Issue deleted"</h2><a class="button-link" href="/issues">"Return to issues"</a></section> })?
        }
        Ok(false) => {
            (view! { <div class="empty-panel"><h2>"Issue not found"</h2><a href="/issues">"Return to issues"</a></div> })?
        }
        Err(error) => {
            if let Some(error) = authorization_error(&error) {
                return Err(error);
            }
            (view! { <div class="error-panel"><strong>"Issue could not be deleted"</strong><p>(error)</p><a href=(format!("/issues/{id}"))>"Return to issue"</a></div> })?
        }
    };
    view! { shell(active: Some(Tab::Issues), title: "Delete issue", keyword: None, child: content) }
}
