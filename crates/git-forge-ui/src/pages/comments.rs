use std::collections::HashMap;

use gix_forge::{Comment, EntityOps, Issue, Review};
use topcoat::{
    Result,
    context::Cx,
    router::{content::Form, page, path_param},
    view::view,
};

use crate::{
    pages::with_repo,
    shell::{Tab, shell},
};

#[path_param(error = bad_request)]
struct CommentId(String);

#[derive(Clone, Default)]
struct CommentForm {
    subject_kind: String,
    subject_id: String,
    author: String,
    body: String,
}

impl CommentForm {
    fn from_input(input: &HashMap<String, String>) -> Self {
        Self {
            subject_kind: input.get("subject_kind").cloned().unwrap_or_default(),
            subject_id: input.get("subject_id").cloned().unwrap_or_default(),
            author: input.get("author").cloned().unwrap_or_default(),
            body: input.get("body").cloned().unwrap_or_default(),
        }
    }
}

async fn comment_form(cx: &Cx, form: &CommentForm, error: Option<&str>) -> Result {
    view! { cx =>
        <section class="form-layout">
            <div class="form-intro">
                <p class="eyebrow">"NEW DISCUSSION"</p>
                <h2>"Leave a note"</h2>
                <p class="muted">"Start a free-floating comment, or attach it to an issue or review."</p>
            </div>
            if let Some(error) = error {
                <div class="error-panel"><strong>"Comment could not be saved"</strong><p>(error)</p></div>
            }
            <form class="entity-form" action="/comments" method="post">
                <fieldset>
                    <legend>"Where should this appear?"</legend>
                    <label for="comment-subject-kind">"Entity type"</label>
                    <select id="comment-subject-kind" name="subject_kind">
                        if form.subject_kind == "issue" {
                            <option value="" >"Free-floating comment"</option>
                            <option value="issue" selected="">"Issue"</option>
                            <option value="review">"Review"</option>
                        } else if form.subject_kind == "review" {
                            <option value="">"Free-floating comment"</option>
                            <option value="issue">"Issue"</option>
                            <option value="review" selected="">"Review"</option>
                        } else {
                            <option value="" selected="">"Free-floating comment"</option>
                            <option value="issue">"Issue"</option>
                            <option value="review">"Review"</option>
                        }
                    </select>
                    <label for="comment-subject-id">"Entity ID <span class=\"muted\">(when attached)</span>"</label>
                    <input id="comment-subject-id" name="subject_id" value=(form.subject_id.as_str()) placeholder="issue ID or review ID">
                </fieldset>
                <fieldset>
                    <legend>"Your note"</legend>
                    <label for="comment-author">"Author"</label>
                    <input id="comment-author" name="author" value=(form.author.as_str()) placeholder="your name" required="">
                    <label for="comment-body">"Comment"</label>
                    <textarea id="comment-body" name="body" rows="8" placeholder="Share context, ask a question, or leave a status update." required="">(form.body.as_str())</textarea>
                </fieldset>
                <div class="form-actions">
                    <a class="button-link secondary" href="/">"Cancel"</a>
                    <button type="submit">"Publish comment"</button>
                </div>
            </form>
        </section>
    }
}

#[page("/comments/new")]
async fn comment_new(cx: &Cx) -> Result {
    let content = comment_form(cx, &CommentForm::default(), None).await?;
    view! { shell(active: Tab::Dashboard, title: "New comment", keyword: None, child: content) }
}

#[page(POST "/comments")]
async fn comment_create(cx: &Cx, Form(input): Form<HashMap<String, String>>) -> Result {
    let form = CommentForm::from_input(&input);
    let author = form.author.trim();
    let body = form.body.trim();
    let subject_kind = form.subject_kind.trim();
    let subject_id = form.subject_id.trim();

    if author.is_empty() || body.is_empty() {
        let content = comment_form(cx, &form, Some("Author and comment are required.")).await?;
        return view! { shell(active: Tab::Dashboard, title: "New comment", keyword: None, child: content) };
    }
    if subject_kind.is_empty() != subject_id.is_empty()
        || (!subject_kind.is_empty() && !matches!(subject_kind, "issue" | "review"))
    {
        let content = comment_form(cx, &form, Some("Choose an issue or review and provide its ID, or leave both attachment fields empty.")).await?;
        return view! { shell(active: Tab::Dashboard, title: "New comment", keyword: None, child: content) };
    }

    let author = author.to_owned();
    let body = body.to_owned();
    let subject_kind = subject_kind.to_owned();
    let subject_id = subject_id.to_owned();
    let result = with_repo(cx, move |repo| {
        if subject_kind.is_empty() {
            Comment {
                id: String::new(),
                subject: None,
                author,
                body,
                binding: None,
                edit: None,
            }
            .create_in_repo(repo)
        } else {
            let exists = match subject_kind.as_str() {
                "issue" => Issue::load_from_repo(repo, &subject_id)?.is_some(),
                "review" => Review::load_from_repo(repo, &subject_id)?.is_some(),
                _ => false,
            };
            if !exists {
                return Err(gix_forge::Error::InvalidTarget(format!(
                    "{subject_kind} `{subject_id}` was not found"
                )));
            }
            Comment::create_under(repo, &subject_kind, &subject_id, &author, &body, None)
        }
    })
    .await;

    match result {
        Ok(id) => {
            let href = format!("/comments/{id}");
            let content = (view! {
                <section class="success-panel">
                    <span class="success-icon">"✓"</span>
                    <p class="eyebrow">"PUBLISHED"</p>
                    <h2>"Your comment is live"</h2>
                    <p class="muted">"It is now part of the repository's forge history."</p>
                    <a class="button-link" href=(href.as_str())>"View comment"</a>
                </section>
            })?;
            view! { shell(active: Tab::Dashboard, title: "Comment published", keyword: None, child: content) }
        }
        Err(error) => {
            let content = comment_form(cx, &form, Some(&error)).await?;
            view! { shell(active: Tab::Dashboard, title: "New comment", keyword: None, child: content) }
        }
    }
}

#[page("/comments/{*comment_id}")]
async fn comment_detail(cx: &Cx) -> Result {
    let id = path_param::<CommentId>(cx)?.clone();
    let data = with_repo(cx, move |repo| Comment::load_from_repo(repo, &id)).await;
    let content = match data {
        Ok(Some(comment)) => {
            let back = comment
                .subject
                .as_deref()
                .and_then(subject_href)
                .unwrap_or_else(|| "/".to_owned());
            (view! {
                <article class="panel narrow-panel">
                    <div class="detail-meta"><span class="status">"comment"</span><span class="muted">"#" (comment.id.as_str())</span></div>
                    <h2>"Comment by " (comment.author.as_str())</h2>
                    if let Some(subject) = &comment.subject {
                        <p class="muted">"Attached to " (subject.as_str())</p>
                    } else {
                        <p class="muted">"Free-floating discussion"</p>
                    }
                    <p class="body-copy">(comment.body.as_str())</p>
                    <div class="form-actions">
                        <a class="button-link secondary" href=(back)>"Back to discussion"</a>
                        <a class="button-link" href=(format!("/comments/edit/{}", comment.id))>"Edit comment"</a>
                        <form class="inline-form" action=(format!("/comments/delete/{}", comment.id)) method="post">
                            <button class="danger-button" type="submit">"Delete"</button>
                        </form>
                    </div>
                </article>
            })?
        }
        Ok(None) => {
            (view! { <div class="empty-panel"><h2>"Comment not found"</h2><a href="/">"Return home"</a></div> })?
        }
        Err(error) => {
            (view! { <div class="error-panel"><strong>"Could not load comment"</strong><p>(error)</p></div> })?
        }
    };
    view! { shell(active: Tab::Dashboard, title: "Comment", keyword: None, child: content) }
}

#[page("/comments/edit/{*comment_id}")]
async fn comment_edit_page(cx: &Cx) -> Result {
    let id = path_param::<CommentId>(cx)?.clone();
    let data = with_repo(cx, move |repo| Comment::load_from_repo(repo, &id)).await;
    let content = match data {
        Ok(Some(comment)) => {
            (view! {
                <section class="form-layout">
                    <div class="form-intro">
                        <p class="eyebrow">"EDIT COMMENT"</p>
                        <h2>"Update your note"</h2>
                        <p class="muted">"Author and attachment stay fixed; only the message changes."</p>
                    </div>
                    <form class="entity-form" action=(format!("/comments/edit/{}", comment.id)) method="post">
                        <p class="form-context"><strong>(comment.author.as_str())</strong> " · " (comment.subject.as_deref().unwrap_or("free-floating"))</p>
                        <label for="comment-edit-body">"Comment"</label>
                        <textarea id="comment-edit-body" name="body" rows="8" required="">(comment.body.as_str())</textarea>
                        <div class="form-actions">
                            <a class="button-link secondary" href=(format!("/comments/{}", comment.id))>"Cancel"</a>
                            <button type="submit">"Save comment"</button>
                        </div>
                    </form>
                </section>
            })?
        }
        Ok(None) => (view! { <div class="empty-panel"><h2>"Comment not found"</h2></div> })?,
        Err(error) => {
            (view! { <div class="error-panel"><strong>"Could not load comment"</strong><p>(error)</p></div> })?
        }
    };
    view! { shell(active: Tab::Dashboard, title: "Edit comment", keyword: None, child: content) }
}

#[page(POST "/comments/edit/{*comment_id}")]
async fn comment_edit_submit(cx: &Cx, Form(input): Form<HashMap<String, String>>) -> Result {
    let id = path_param::<CommentId>(cx)?.clone();
    let body = input
        .get("body")
        .map(String::as_str)
        .unwrap_or_default()
        .trim();
    if body.is_empty() {
        let content = (view! {
            <div class="error-panel"><strong>"Comment could not be saved"</strong><p>"A comment cannot be empty."</p><a href=(format!("/comments/edit/{id}"))>"Return to editor"</a></div>
        })?;
        return view! { shell(active: Tab::Dashboard, title: "Edit comment", keyword: None, child: content) };
    }

    let body = body.to_owned();
    let lookup_id = id.clone();
    let result = with_repo(cx, move |repo| {
        let mut comment = Comment::load_from_repo(repo, &lookup_id)?.ok_or_else(|| {
            gix_forge::Error::InvalidTarget(format!("comment `{lookup_id}` was not found"))
        })?;
        comment.body = body;
        comment.save_in_repo(repo).map(|_| comment.id)
    })
    .await;

    match result {
        Ok(id) => {
            let content = (view! {
                <section class="success-panel">
                    <span class="success-icon">"✓"</span>
                    <p class="eyebrow">"SAVED"</p>
                    <h2>"Comment updated"</h2>
                    <a class="button-link" href=(format!("/comments/{id}"))>"View comment"</a>
                </section>
            })?;
            view! { shell(active: Tab::Dashboard, title: "Comment updated", keyword: None, child: content) }
        }
        Err(error) => {
            let content = (view! { <div class="error-panel"><strong>"Comment could not be saved"</strong><p>(error)</p><a href=(format!("/comments/edit/{id}"))>"Return to editor"</a></div> })?;
            view! { shell(active: Tab::Dashboard, title: "Edit comment", keyword: None, child: content) }
        }
    }
}

#[page(POST "/comments/delete/{*comment_id}")]
async fn comment_delete(cx: &Cx) -> Result {
    let id = path_param::<CommentId>(cx)?.clone();
    let lookup_id = id.clone();
    let result = with_repo(cx, move |repo| Comment::delete(repo, &lookup_id)).await;
    let content = match result {
        Ok(true) => {
            (view! { <section class="success-panel"><span class="success-icon">"✓"</span><h2>"Comment deleted"</h2><a class="button-link" href="/">"Return home"</a></section> })?
        }
        Ok(false) => {
            (view! { <div class="empty-panel"><h2>"Comment not found"</h2><a href="/">"Return home"</a></div> })?
        }
        Err(error) => {
            (view! { <div class="error-panel"><strong>"Comment could not be deleted"</strong><p>(error)</p><a href=(format!("/comments/{id}"))>"Return to comment"</a></div> })?
        }
    };
    view! { shell(active: Tab::Dashboard, title: "Delete comment", keyword: None, child: content) }
}

fn subject_href(subject: &str) -> Option<String> {
    let (kind, id) = subject.split_once(':')?;
    matches!(kind, "issue" | "review").then(|| format!("/{kind}s/{id}"))
}
