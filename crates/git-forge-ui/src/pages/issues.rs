use std::collections::HashMap;

use gix_forge::{Commentable, EntityOps, Issue, Status};
use topcoat::{
    Result,
    context::Cx,
    router::{content::Form, page, path_param},
    view::{Unescaped, view},
};

use crate::{
    pages::{csv_values, with_repo},
    render::render_asciidoc,
    shell::{Tab, shell, split_shell},
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
                    <label for="issue-body">"Description <span class=\"muted\">(optional)</span>"</label>
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
                    <label for="issue-labels">"Labels <span class=\"muted\">(comma-separated)</span>"</label>
                    <input id="issue-labels" name="labels" value=(form.labels.as_str()) placeholder="bug, documentation">
                    <label for="issue-assignees">"Assignees <span class=\"muted\">(comma-separated)</span>"</label>
                    <input id="issue-assignees" name="assignees" value=(form.assignees.as_str()) placeholder="handle or member ID">
                    <label for="issue-reporters">"Reporters <span class=\"muted\">(comma-separated)</span>"</label>
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

async fn issue_list(cx: &Cx, items: &[Issue]) -> Result {
    view! { cx =>
        <div class="list-heading">
            <div><h2>"Issues"</h2><span class="muted">(items.len())</span></div>
            <a class="button-link" href="/issues/new">"New issue"</a>
        </div>
        if items.is_empty() {
            <div class="empty-panel"><p>"No issues found."</p><a class="button-link" href="/issues/new">"Open the first issue"</a></div>
        } else {
            <ul class="entity-list">
                for issue in items {
                    <li>
                        <a class="entity-link" href=(format!("/issues/{}", issue.id))>
                            <span>
                                <strong>(issue.title.as_str())</strong>
                                <small>(issue.id.as_str())</small>
                            </span>
                            <span class="status">(issue.status.as_str())</span>
                        </a>
                    </li>
                }
            </ul>
        }
    }
}

#[page("/issues")]
async fn issues(cx: &Cx) -> Result {
    let data = with_repo(cx, |repo| {
        let ids = Issue::list(repo)?;
        ids.into_iter()
            .map(|id| Issue::load_from_repo(repo, &id))
            .collect::<Result<Vec<_>, _>>()
            .map(|issues| issues.into_iter().flatten().collect::<Vec<Issue>>())
    })
    .await;

    let (list, detail) = match data {
        Ok(issues) => (
            issue_list(cx, &issues).await?,
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

    view! { split_shell(active: Tab::Issues, title: "Issues", list: list, detail: detail) }
}

#[page("/issues/new")]
async fn issue_new(cx: &Cx) -> Result {
    let form = IssueForm {
        title: String::new(),
        body: String::new(),
        status: "open".to_owned(),
        labels: String::new(),
        assignees: String::new(),
        reporters: String::new(),
    };
    let content = issue_form(cx, "/issues", &form, None).await?;
    view! { shell(active: Tab::Issues, title: "New issue", keyword: None, child: content) }
}

#[page(POST "/issues")]
async fn issue_create(cx: &Cx, Form(input): Form<HashMap<String, String>>) -> Result {
    let form = IssueForm::from_input(&input);
    let title = form.title.trim();
    if title.is_empty() {
        let content = issue_form(cx, "/issues", &form, Some("A title is required.")).await?;
        return view! { shell(active: Tab::Issues, title: "New issue", keyword: None, child: content) };
    }
    if Status::parse(&form.status).is_none() {
        let content = issue_form(
            cx,
            "/issues",
            &form,
            Some("Choose either Open or Closed for the status."),
        )
        .await?;
        return view! { shell(active: Tab::Issues, title: "New issue", keyword: None, child: content) };
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
    let result = with_repo(cx, move |repo| issue.create_in_repo(repo)).await;
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
            view! { shell(active: Tab::Issues, title: "Issue created", keyword: None, child: content) }
        }
        Err(error) => {
            let content = issue_form(cx, "/issues", &form, Some(&error)).await?;
            view! { shell(active: Tab::Issues, title: "New issue", keyword: None, child: content) }
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
                let ids = Issue::list(repo)?;
                let issue_items = ids
                    .into_iter()
                    .map(|id| Issue::load_from_repo(repo, &id))
                    .collect::<Result<Vec<_>, _>>()?
                    .into_iter()
                    .flatten()
                    .collect::<Vec<Issue>>();
                Ok(Some((issue_items, issue, comments)))
            }
            None => Ok(None),
        }
    })
    .await;

    let (list, detail) = match data {
        Ok(Some((issue_items, issue, comments))) => {
            let next_status = if issue.status == "open" {
                "closed"
            } else {
                "open"
            };
            (
                issue_list(cx, &issue_items).await?,
                (view! {
                    <article>
                        <div class="detail-meta">
                            <span class="status">(issue.status.as_str())</span>
                            <span class="muted">"#" (issue.id.as_str())</span>
                        </div>
                        <div class="detail-heading">
                            <div><h2>(issue.title.as_str())</h2><p class="muted">"Issue " (issue.id.as_str())</p></div>
                            <div class="detail-actions">
                                <a class="button-link secondary" href=(format!("/issues/{}/edit", issue.id))>"Edit"</a>
                                <form class="inline-form" action=(format!("/issues/{}/status", issue.id)) method="post">
                                    <input type="hidden" name="status" value=(next_status)>
                                    <button type="submit">if next_status == "closed" { "Close issue" } else { "Reopen issue" }</button>
                                </form>
                                <form class="inline-form" action=(format!("/issues/{}/delete", issue.id)) method="post">
                                    <button class="danger-button" type="submit">"Delete"</button>
                                </form>
                            </div>
                        </div>
                        <div class="body-copy rendered-file">(Unescaped::new_unchecked(render_asciidoc(&issue.body)))</div>
                        <div class="tag-row">
                            for label in &issue.labels { <span class="tag">(label.as_str())</span> }
                        </div>
                        <dl class="facts">
                            <dt>"Assignees"</dt><dd>(if issue.assignees.is_empty() { "None".to_owned() } else { issue.assignees.join(", ") })</dd>
                            <dt>"Reporters"</dt><dd>(if issue.reporters.is_empty() { "None".to_owned() } else { issue.reporters.join(", ") })</dd>
                        </dl>
                        <section class="comments">
                            <div class="panel-heading"><div><h3>"Comments"</h3><span class="muted">(comments.len())</span></div><a class="button-link secondary" href="/comments/new">"Free-floating comment"</a></div>
                            if comments.is_empty() {
                                <p class="empty">"No comments yet. Start the conversation below."</p>
                            } else {
                                for comment in &comments {
                                    <article class="comment">
                                        <div class="comment-heading"><strong>(comment.author.as_str())</strong><a class="muted" href=(format!("/comments/{}/edit", comment.id))>"Edit"</a></div>
                                        <p>(comment.body.as_str())</p>
                                    </article>
                                }
                            }
                            <form class="comment-form" action=(format!("/issues/{}/comments", issue.id)) method="post">
                                <label for="issue-comment-author">"Add a comment"</label>
                                <input id="issue-comment-author" name="author" placeholder="Your name" required="">
                                <textarea name="body" rows="5" placeholder="Leave a helpful update or question." required=""></textarea>
                                <button type="submit">"Comment"</button>
                            </form>
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

    view! { split_shell(active: Tab::Issues, title: "Issue", list: list, detail: detail) }
}

#[page("/issues/{issue_id}/edit")]
async fn issue_edit_page(cx: &Cx) -> Result {
    let id = path_param::<IssueId>(cx)?.clone();
    let data = with_repo(cx, move |repo| Issue::load_from_repo(repo, &id)).await;
    let content = match data {
        Ok(Some(issue)) => {
            issue_form(
                cx,
                &format!("/issues/{}/edit", issue.id),
                &IssueForm::from_issue(&issue),
                None,
            )
            .await?
        }
        Ok(None) => {
            (view! { <div class="empty-panel"><h2>"Issue not found"</h2><a href="/issues">"Back to issues"</a></div> })?
        }
        Err(error) => {
            (view! { <div class="error-panel"><strong>"Could not load issue"</strong><p>(error)</p></div> })?
        }
    };
    view! { shell(active: Tab::Issues, title: "Edit issue", keyword: None, child: content) }
}

#[page(POST "/issues/{issue_id}/edit")]
async fn issue_edit_submit(cx: &Cx, Form(input): Form<HashMap<String, String>>) -> Result {
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
        return view! { shell(active: Tab::Issues, title: "Edit issue", keyword: None, child: content) };
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
        issue.reporters = csv_values(&form.reporters);
        issue.save_in_repo(repo).map(|_| issue.id)
    })
    .await;
    match result {
        Ok(id) => {
            let content = (view! { <section class="success-panel"><span class="success-icon">"✓"</span><p class="eyebrow">"SAVED"</p><h2>"Issue updated"</h2><a class="button-link" href=(format!("/issues/{id}"))>"View issue"</a></section> })?;
            view! { shell(active: Tab::Issues, title: "Issue updated", keyword: None, child: content) }
        }
        Err(error) => {
            let content = (view! { <div class="error-panel"><strong>"Issue could not be saved"</strong><p>(error)</p><a href=(format!("/issues/{id}/edit"))>"Return to editor"</a></div> })?;
            view! { shell(active: Tab::Issues, title: "Edit issue", keyword: None, child: content) }
        }
    }
}

#[page(POST "/issues/{issue_id}/status")]
async fn issue_status(cx: &Cx, Form(input): Form<HashMap<String, String>>) -> Result {
    let id = path_param::<IssueId>(cx)?.clone();
    let status = input
        .get("status")
        .map(String::as_str)
        .unwrap_or_default()
        .to_owned();
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
        issue.save_in_repo(repo).map(|_| issue.id)
    })
    .await;
    match result {
        Ok(id) => {
            let content = (view! { <section class="success-panel"><span class="success-icon">"✓"</span><h2>"Issue status updated"</h2><a class="button-link" href=(format!("/issues/{id}"))>"Return to issue"</a></section> })?;
            view! { shell(active: Tab::Issues, title: "Issue updated", keyword: None, child: content) }
        }
        Err(error) => {
            let content = (view! { <div class="error-panel"><strong>"Could not update issue"</strong><p>(error)</p><a href=(format!("/issues/{id}"))>"Return to issue"</a></div> })?;
            view! { shell(active: Tab::Issues, title: "Issue action failed", keyword: None, child: content) }
        }
    }
}

#[page(POST "/issues/{issue_id}/comments")]
async fn issue_comment(cx: &Cx, Form(input): Form<HashMap<String, String>>) -> Result {
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
        let content = (view! { <div class="error-panel"><strong>"Comment could not be added"</strong><p>"Your name and a comment are required."</p><a href=(format!("/issues/{id}"))>"Return to issue"</a></div> })?;
        return view! { shell(active: Tab::Issues, title: "Comment failed", keyword: None, child: content) };
    }
    let lookup_id = id.clone();
    let result = with_repo(cx, move |repo| {
        let issue = Issue::load_from_repo(repo, &lookup_id)?.ok_or_else(|| {
            gix_forge::Error::InvalidTarget(format!("issue `{lookup_id}` was not found"))
        })?;
        issue.add_comment(repo, &author, &body)
    })
    .await;
    match result {
        Ok(_) => {
            let content = (view! { <section class="success-panel"><span class="success-icon">"✓"</span><h2>"Comment added"</h2><a class="button-link" href=(format!("/issues/{id}"))>"Return to issue"</a></section> })?;
            view! { shell(active: Tab::Issues, title: "Comment added", keyword: None, child: content) }
        }
        Err(error) => {
            let content = (view! { <div class="error-panel"><strong>"Comment could not be added"</strong><p>(error)</p><a href=(format!("/issues/{id}"))>"Return to issue"</a></div> })?;
            view! { shell(active: Tab::Issues, title: "Comment failed", keyword: None, child: content) }
        }
    }
}

#[page(POST "/issues/{issue_id}/delete")]
async fn issue_delete(cx: &Cx) -> Result {
    let id = path_param::<IssueId>(cx)?.clone();
    let lookup_id = id.clone();
    let result = with_repo(cx, move |repo| Issue::delete(repo, &lookup_id)).await;
    let content = match result {
        Ok(true) => {
            (view! { <section class="success-panel"><span class="success-icon">"✓"</span><h2>"Issue deleted"</h2><a class="button-link" href="/issues">"Return to issues"</a></section> })?
        }
        Ok(false) => {
            (view! { <div class="empty-panel"><h2>"Issue not found"</h2><a href="/issues">"Return to issues"</a></div> })?
        }
        Err(error) => {
            (view! { <div class="error-panel"><strong>"Issue could not be deleted"</strong><p>(error)</p><a href=(format!("/issues/{id}"))>"Return to issue"</a></div> })?
        }
    };
    view! { shell(active: Tab::Issues, title: "Delete issue", keyword: None, child: content) }
}
