use gix_forge::{Commentable, EntityOps, Review};
use topcoat::{
    Result,
    context::Cx,
    router::{page, path_param},
    view::{Unescaped, view},
};

use crate::{
    pages::with_repo,
    render::render_asciidoc,
    shell::{Tab, split_shell},
};

#[path_param(error = bad_request)]
struct ReviewId(String);

async fn review_list(cx: &Cx, items: &[Review]) -> Result {
    view! { cx =>
        <div class="list-heading">
            <h2>"Reviews"</h2>
            <span class="muted">(items.len())</span>
        </div>
        if items.is_empty() {
            <p class="empty">"No reviews found."</p>
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
                    <p class="muted">"Choose a review from the list to read its request and comments."</p>
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
        Ok(Some((review_items, review, comments))) => (
            review_list(cx, &review_items).await?,
            (view! {
                <article>
                    <div class="detail-meta">
                        <span class="status">(review.status.as_str())</span>
                        <span class="muted">"#" (review.id.as_str())</span>
                    </div>
                    <h2>"Review " (review.id.as_str())</h2>
                    <div class="body-copy rendered-file">(Unescaped::new_unchecked(render_asciidoc(&review.body)))</div>
                    <dl class="facts">
                        <dt>"Requesters"</dt>
                        <dd>(review.requesters.join(", "))</dd>
                        <dt>"Reviewers"</dt>
                        <dd>(review.reviewers.join(", "))</dd>
                        <dt>"Target"</dt>
                        <dd>(format!("{:?}", review.target))</dd>
                    </dl>
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
