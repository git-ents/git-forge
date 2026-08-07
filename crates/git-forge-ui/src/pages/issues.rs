use gix_forge::{Commentable, EntityOps, Issue};
use topcoat::{
    Result,
    context::Cx,
    router::{page, path_param},
    view::view,
};

use crate::{
    pages::with_repo,
    shell::{Tab, split_shell},
};

#[path_param(error = bad_request)]
struct IssueId(String);

async fn issue_list(cx: &Cx, items: &[Issue]) -> Result {
    view! { cx =>
        <div class="list-heading">
            <h2>"Issues"</h2>
            <span class="muted">(items.len())</span>
        </div>
        if items.is_empty() {
            <p class="empty">"No issues found."</p>
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
                    <p class="muted">"Choose an issue from the list to read its body and comments."</p>
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

#[page("/issues/{id}")]
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
        Ok(Some((issue_items, issue, comments))) => (
            issue_list(cx, &issue_items).await?,
            (view! {
                <article>
                    <div class="detail-meta">
                        <span class="status">(issue.status.as_str())</span>
                        <span class="muted">"#" (issue.id.as_str())</span>
                    </div>
                    <h2>(issue.title.as_str())</h2>
                    <p class="body-copy">(issue.body.as_str())</p>
                    <div class="tag-row">
                        for label in &issue.labels {
                            <span class="tag">(label.as_str())</span>
                        }
                    </div>
                    <section class="comments">
                        <div class="panel-heading">
                            <h3>"Comments"</h3>
                            <span class="muted">(comments.len())</span>
                        </div>
                        if comments.is_empty() {
                            <p class="empty">"No comments."</p>
                        } else {
                            for comment in &comments {
                                <article class="comment">
                                    <strong>(comment.author.as_str())</strong>
                                    <p>(comment.body.as_str())</p>
                                </article>
                            }
                        }
                    </section>
                </article>
            })?,
        ),
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
