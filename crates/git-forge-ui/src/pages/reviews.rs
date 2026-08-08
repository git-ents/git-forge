use std::collections::HashMap;

use gix_forge::{Commentable, EntityOps, Review, ReviewTarget, Status};
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
struct ReviewId(String);

#[derive(Clone)]
struct ReviewForm {
    body: String,
    status: String,
    reviewers: String,
    requesters: String,
    target: String,
}

impl ReviewForm {
    fn from_review(review: &Review) -> Self {
        Self {
            body: review.body.clone(),
            status: review.status.clone(),
            reviewers: review.reviewers.join(", "),
            requesters: review.requesters.join(", "),
            target: review.target.to_string(),
        }
    }

    fn from_input(input: &HashMap<String, String>) -> Self {
        Self {
            body: input.get("body").cloned().unwrap_or_default(),
            status: input
                .get("status")
                .cloned()
                .unwrap_or_else(|| "open".to_owned()),
            reviewers: input.get("reviewers").cloned().unwrap_or_default(),
            requesters: input.get("requesters").cloned().unwrap_or_default(),
            target: input.get("target").cloned().unwrap_or_default(),
        }
    }
}

async fn review_form(cx: &Cx, action: &str, form: &ReviewForm, error: Option<&str>) -> Result {
    view! { cx =>
        <section class="form-layout">
            <div class="form-intro">
                <p class="eyebrow">"CODE REVIEW"</p>
                <h2>"Ask for a second set of eyes"</h2>
                <p class="muted">"Describe what should be reviewed and make the target easy to identify."</p>
            </div>
            if let Some(error) = error {
                <div class="error-panel"><strong>"Review could not be saved"</strong><p>(error)</p></div>
            }
            <form class="entity-form" action=(action) method="post">
                <fieldset>
                    <legend>"Review request"</legend>
                    <label for="review-body">"Summary <span class=\"muted\">(optional)</span>"</label>
                    <textarea id="review-body" name="body" rows="9" placeholder="What would you like reviewers to focus on?">(form.body.as_str())</textarea>
                    <label for="review-target">"Target"</label>
                    <input id="review-target" name="target" value=(form.target.as_str()) placeholder="commit:abc123 or commit-range:base:tip" required="">
                    <p class="form-help">"Use commit:, tree:, blob:path:oid, base-tip-tree:, base-tip-commit:, or commit-range:. A bare value is treated as a commit."</p>
                </fieldset>
                <fieldset>
                    <legend>"People and status"</legend>
                    <label for="review-status">"Status"</label>
                    <select id="review-status" name="status">
                        if form.status == "closed" {
                            <option value="open">"Open"</option>
                            <option value="closed" selected="">"Closed"</option>
                        } else {
                            <option value="open" selected="">"Open"</option>
                            <option value="closed">"Closed"</option>
                        }
                    </select>
                    <label for="review-reviewers">"Reviewers <span class=\"muted\">(comma-separated)</span>"</label>
                    <input id="review-reviewers" name="reviewers" value=(form.reviewers.as_str()) placeholder="handle or member ID">
                    <label for="review-requesters">"Requesters <span class=\"muted\">(comma-separated)</span>"</label>
                    <input id="review-requesters" name="requesters" value=(form.requesters.as_str()) placeholder="handle or member ID">
                </fieldset>
                <div class="form-actions">
                    <a class="button-link secondary" href="/reviews">"Cancel"</a>
                    <button type="submit">"Save review"</button>
                </div>
            </form>
        </section>
    }
}

async fn review_list(cx: &Cx, items: &[Review]) -> Result {
    view! { cx =>
        <div class="list-heading">
            <div><h2>"Reviews"</h2><span class="muted">(items.len())</span></div>
            <a class="button-link" href="/reviews/new">"New review"</a>
        </div>
        if items.is_empty() {
            <div class="empty-panel"><p>"No reviews found."</p><a class="button-link" href="/reviews/new">"Request a review"</a></div>
        } else {
            <ul class="entity-list">
                for review in items {
                    <li>
                        <a class="entity-link" href=(format!("/reviews/{}", review.id))>
                            <span>
                                <strong>"Review " (review.id.as_str())</strong>
                                <small>(review.requesters.join(", "))</small>
                            </span>
                            <span class="status">(review.status.as_str())</span>
                        </a>
                    </li>
                }
            </ul>
        }
    }
}

#[page("/reviews")]
async fn reviews(cx: &Cx) -> Result {
    let data = with_repo(cx, |repo| {
        let ids = Review::list(repo)?;
        ids.into_iter()
            .map(|id| Review::load_from_repo(repo, &id))
            .collect::<Result<Vec<_>, _>>()
            .map(|reviews| reviews.into_iter().flatten().collect::<Vec<Review>>())
    })
    .await;

    let (list, detail) = match data {
        Ok(reviews) => (
            review_list(cx, &reviews).await?,
            (view! {
                <div class="placeholder-detail">
                    <p class="eyebrow">"REVIEW DETAIL"</p>
                    <h2>"Select a review"</h2>
                    <p class="muted">"Choose a review from the list to read its request and comments, or create a new request."</p>
                </div>
            })?,
        ),
        Err(error) => (
            (view! { <p class="error-text">(error)</p> })?,
            (view! { <p class="error-text">"Could not load reviews."</p> })?,
        ),
    };

    view! { split_shell(active: Tab::Reviews, title: "Reviews", list: list, detail: detail) }
}

#[page("/reviews/new")]
async fn review_new(cx: &Cx) -> Result {
    let form = ReviewForm {
        body: String::new(),
        status: "open".to_owned(),
        reviewers: String::new(),
        requesters: String::new(),
        target: String::new(),
    };
    let content = review_form(cx, "/reviews", &form, None).await?;
    view! { shell(active: Tab::Reviews, title: "New review", keyword: None, child: content) }
}

#[page(POST "/reviews")]
async fn review_create(cx: &Cx, Form(input): Form<HashMap<String, String>>) -> Result {
    let form = ReviewForm::from_input(&input);
    if form.target.trim().is_empty() || ReviewTarget::parse(form.target.trim()).is_err() {
        let content = review_form(
            cx,
            "/reviews",
            &form,
            Some("Enter a valid review target using one of the supported target formats."),
        )
        .await?;
        return view! { shell(active: Tab::Reviews, title: "New review", keyword: None, child: content) };
    }
    if Status::parse(&form.status).is_none() {
        let content = review_form(
            cx,
            "/reviews",
            &form,
            Some("Choose either Open or Closed for the status."),
        )
        .await?;
        return view! { shell(active: Tab::Reviews, title: "New review", keyword: None, child: content) };
    }
    let review = Review {
        id: String::new(),
        status: form.status.trim().to_ascii_lowercase(),
        body: form.body.trim().to_owned(),
        reviewers: csv_values(&form.reviewers),
        requesters: csv_values(&form.requesters),
        target: ReviewTarget::parse(form.target.trim()).expect("validated review target"),
        edit: None,
    };
    let result = with_repo(cx, move |repo| review.create_in_repo(repo)).await;
    match result {
        Ok(id) => {
            let content = (view! { <section class="success-panel"><span class="success-icon">"✓"</span><p class="eyebrow">"REVIEW CREATED"</p><h2>"Your review request is ready"</h2><p class="muted">"Reviewers can now find the target and leave feedback."</p><a class="button-link" href=(format!("/reviews/{id}"))>"View review"</a></section> })?;
            view! { shell(active: Tab::Reviews, title: "Review created", keyword: None, child: content) }
        }
        Err(error) => {
            let content = review_form(cx, "/reviews", &form, Some(&error)).await?;
            view! { shell(active: Tab::Reviews, title: "New review", keyword: None, child: content) }
        }
    }
}

#[page("/reviews/{review_id}")]
async fn review_detail(cx: &Cx) -> Result {
    let id = path_param::<ReviewId>(cx)?.clone();
    let data = with_repo(cx, move |repo| {
        let review = Review::load_from_repo(repo, &id)?;
        match review {
            Some(review) => {
                let comments = review.get_comments(repo)?;
                let ids = Review::list(repo)?;
                let review_items = ids
                    .into_iter()
                    .map(|id| Review::load_from_repo(repo, &id))
                    .collect::<Result<Vec<_>, _>>()?
                    .into_iter()
                    .flatten()
                    .collect::<Vec<Review>>();
                Ok(Some((review_items, review, comments)))
            }
            None => Ok(None),
        }
    })
    .await;

    let (list, detail) = match data {
        Ok(Some((review_items, review, comments))) => {
            let next_status = if review.status == "open" {
                "closed"
            } else {
                "open"
            };
            (
                review_list(cx, &review_items).await?,
                (view! {
                    <article>
                        <div class="detail-meta"><span class="status">(review.status.as_str())</span><span class="muted">"#" (review.id.as_str())</span></div>
                        <div class="detail-heading">
                            <div><h2>"Review " (review.id.as_str())</h2><p class="muted">"A request for focused feedback"</p></div>
                            <div class="detail-actions">
                                <a class="button-link secondary" href=(format!("/reviews/{}/edit", review.id))>"Edit"</a>
                                <form class="inline-form" action=(format!("/reviews/{}/status", review.id)) method="post"><input type="hidden" name="status" value=(next_status)><button type="submit">if next_status == "closed" { "Close review" } else { "Reopen review" }</button></form>
                                <form class="inline-form" action=(format!("/reviews/{}/delete", review.id)) method="post"><button class="danger-button" type="submit">"Delete"</button></form>
                            </div>
                        </div>
                        <div class="body-copy rendered-file">(Unescaped::new_unchecked(render_asciidoc(&review.body)))</div>
                        <dl class="facts">
                            <dt>"Requesters"</dt><dd>(if review.requesters.is_empty() { "None".to_owned() } else { review.requesters.join(", ") })</dd>
                            <dt>"Reviewers"</dt><dd>(if review.reviewers.is_empty() { "None".to_owned() } else { review.reviewers.join(", ") })</dd>
                            <dt>"Target"</dt><dd><code>(review.target.to_string())</code></dd>
                        </dl>
                        <section class="comments">
                            <div class="panel-heading"><div><h3>"Comments"</h3><span class="muted">(comments.len())</span></div><a class="button-link secondary" href="/comments/new">"Free-floating comment"</a></div>
                            if comments.is_empty() {
                                <p class="empty">"No comments yet. Start the conversation below."</p>
                            } else {
                                for comment in &comments {
                                    <article class="comment"><div class="comment-heading"><strong>(comment.author.as_str())</strong><a class="muted" href=(format!("/comments/edit/{}", comment.id))>"Edit"</a></div><p>(comment.body.as_str())</p></article>
                                }
                            }
                            <form class="comment-form" action=(format!("/reviews/{}/comments", review.id)) method="post">
                                <label for="review-comment-author">"Add a comment"</label>
                                <input id="review-comment-author" name="author" placeholder="Your name" required="">
                                <textarea name="body" rows="5" placeholder="Leave a helpful update or question." required=""></textarea>
                                <button type="submit">"Comment"</button>
                            </form>
                        </section>
                    </article>
                })?,
            )
        }
        Ok(None) => (
            (view! { <p class="empty">"Review not found."</p> })?,
            (view! { <p class="empty">"Review not found."</p> })?,
        ),
        Err(error) => (
            (view! { <p class="error-text">(error)</p> })?,
            (view! { <p class="error-text">"Could not load review."</p> })?,
        ),
    };

    view! { split_shell(active: Tab::Reviews, title: "Review", list: list, detail: detail) }
}

#[page("/reviews/{review_id}/edit")]
async fn review_edit_page(cx: &Cx) -> Result {
    let id = path_param::<ReviewId>(cx)?.clone();
    let data = with_repo(cx, move |repo| Review::load_from_repo(repo, &id)).await;
    let content = match data {
        Ok(Some(review)) => {
            review_form(
                cx,
                &format!("/reviews/{}/edit", review.id),
                &ReviewForm::from_review(&review),
                None,
            )
            .await?
        }
        Ok(None) => {
            (view! { <div class="empty-panel"><h2>"Review not found"</h2><a href="/reviews">"Back to reviews"</a></div> })?
        }
        Err(error) => {
            (view! { <div class="error-panel"><strong>"Could not load review"</strong><p>(error)</p></div> })?
        }
    };
    view! { shell(active: Tab::Reviews, title: "Edit review", keyword: None, child: content) }
}

#[page(POST "/reviews/{review_id}/edit")]
async fn review_edit_submit(cx: &Cx, Form(input): Form<HashMap<String, String>>) -> Result {
    let id = path_param::<ReviewId>(cx)?.clone();
    let form = ReviewForm::from_input(&input);
    if form.target.trim().is_empty()
        || ReviewTarget::parse(form.target.trim()).is_err()
        || Status::parse(&form.status).is_none()
    {
        let content = review_form(
            cx,
            &format!("/reviews/{id}/edit"),
            &form,
            Some("Target and status must use supported values."),
        )
        .await?;
        return view! { shell(active: Tab::Reviews, title: "Edit review", keyword: None, child: content) };
    }
    let lookup_id = id.clone();
    let result = with_repo(cx, move |repo| {
        let mut review = Review::load_from_repo(repo, &lookup_id)?.ok_or_else(|| {
            gix_forge::Error::InvalidTarget(format!("review `{lookup_id}` was not found"))
        })?;
        review.body = form.body.trim().to_owned();
        review.status = form.status.trim().to_ascii_lowercase();
        review.reviewers = csv_values(&form.reviewers);
        review.requesters = csv_values(&form.requesters);
        review.target = ReviewTarget::parse(form.target.trim()).expect("validated review target");
        review.save_in_repo(repo).map(|_| review.id)
    })
    .await;
    match result {
        Ok(id) => {
            let content = (view! { <section class="success-panel"><span class="success-icon">"✓"</span><p class="eyebrow">"SAVED"</p><h2>"Review updated"</h2><a class="button-link" href=(format!("/reviews/{id}"))>"View review"</a></section> })?;
            view! { shell(active: Tab::Reviews, title: "Review updated", keyword: None, child: content) }
        }
        Err(error) => {
            let content = (view! { <div class="error-panel"><strong>"Review could not be saved"</strong><p>(error)</p><a href=(format!("/reviews/{id}/edit"))>"Return to editor"</a></div> })?;
            view! { shell(active: Tab::Reviews, title: "Edit review", keyword: None, child: content) }
        }
    }
}

#[page(POST "/reviews/{review_id}/status")]
async fn review_status(cx: &Cx, Form(input): Form<HashMap<String, String>>) -> Result {
    let id = path_param::<ReviewId>(cx)?.clone();
    let status = input
        .get("status")
        .map(String::as_str)
        .unwrap_or_default()
        .to_owned();
    let lookup_id = id.clone();
    let result = with_repo(cx, move |repo| {
        let mut review = Review::load_from_repo(repo, &lookup_id)?.ok_or_else(|| {
            gix_forge::Error::InvalidTarget(format!("review `{lookup_id}` was not found"))
        })?;
        if Status::parse(&status).is_none() {
            return Err(gix_forge::Error::InvalidTarget(
                "invalid review status".to_owned(),
            ));
        }
        review.status = status;
        review.save_in_repo(repo).map(|_| review.id)
    })
    .await;
    match result {
        Ok(id) => {
            let content = (view! { <section class="success-panel"><span class="success-icon">"✓"</span><h2>"Review status updated"</h2><a class="button-link" href=(format!("/reviews/{id}"))>"Return to review"</a></section> })?;
            view! { shell(active: Tab::Reviews, title: "Review updated", keyword: None, child: content) }
        }
        Err(error) => {
            let content = (view! { <div class="error-panel"><strong>"Could not update review"</strong><p>(error)</p><a href=(format!("/reviews/{id}"))>"Return to review"</a></div> })?;
            view! { shell(active: Tab::Reviews, title: "Review action failed", keyword: None, child: content) }
        }
    }
}

#[page(POST "/reviews/{review_id}/comments")]
async fn review_comment(cx: &Cx, Form(input): Form<HashMap<String, String>>) -> Result {
    let id = path_param::<ReviewId>(cx)?.clone();
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
        let content = (view! { <div class="error-panel"><strong>"Comment could not be added"</strong><p>"Your name and a comment are required."</p><a href=(format!("/reviews/{id}"))>"Return to review"</a></div> })?;
        return view! { shell(active: Tab::Reviews, title: "Comment failed", keyword: None, child: content) };
    }
    let lookup_id = id.clone();
    let result = with_repo(cx, move |repo| {
        let review = Review::load_from_repo(repo, &lookup_id)?.ok_or_else(|| {
            gix_forge::Error::InvalidTarget(format!("review `{lookup_id}` was not found"))
        })?;
        review.add_comment(repo, &author, &body)
    })
    .await;
    match result {
        Ok(_) => {
            let content = (view! { <section class="success-panel"><span class="success-icon">"✓"</span><h2>"Comment added"</h2><a class="button-link" href=(format!("/reviews/{id}"))>"Return to review"</a></section> })?;
            view! { shell(active: Tab::Reviews, title: "Comment added", keyword: None, child: content) }
        }
        Err(error) => {
            let content = (view! { <div class="error-panel"><strong>"Comment could not be added"</strong><p>(error)</p><a href=(format!("/reviews/{id}"))>"Return to review"</a></div> })?;
            view! { shell(active: Tab::Reviews, title: "Comment failed", keyword: None, child: content) }
        }
    }
}

#[page(POST "/reviews/{review_id}/delete")]
async fn review_delete(cx: &Cx) -> Result {
    let id = path_param::<ReviewId>(cx)?.clone();
    let lookup_id = id.clone();
    let result = with_repo(cx, move |repo| Review::delete(repo, &lookup_id)).await;
    let content = match result {
        Ok(true) => {
            (view! { <section class="success-panel"><span class="success-icon">"✓"</span><h2>"Review deleted"</h2><a class="button-link" href="/reviews">"Return to reviews"</a></section> })?
        }
        Ok(false) => {
            (view! { <div class="empty-panel"><h2>"Review not found"</h2><a href="/reviews">"Return to reviews"</a></div> })?
        }
        Err(error) => {
            (view! { <div class="error-panel"><strong>"Review could not be deleted"</strong><p>(error)</p><a href=(format!("/reviews/{id}"))>"Return to review"</a></div> })?
        }
    };
    view! { shell(active: Tab::Reviews, title: "Delete review", keyword: None, child: content) }
}
