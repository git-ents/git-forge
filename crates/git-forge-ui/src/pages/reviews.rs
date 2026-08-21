use std::collections::HashMap;

use gix_forge::{
    Comment, Commentable, EntityOps, Member, Review, ReviewDecision, ReviewTarget, ReviewVerdict,
    Status,
};
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

/// The design's six git-anchor kinds, collapsed from `ReviewTarget`'s six
/// variants: label text plus the DESIGN-SPEC token class that colors it
/// (`oid` for blob-shaped targets, `warn` for tree-shaped, `mauve` for
/// commit-shaped -- "Anchor kind colors" in DESIGN-SPEC.md).
fn anchor_kind(target: &ReviewTarget) -> (&'static str, &'static str) {
    match target {
        ReviewTarget::Blobs(_) => ("blob", "oid"),
        ReviewTarget::Trees(_) => ("tree", "warn"),
        ReviewTarget::Commits(_) => ("commit", "mauve"),
        ReviewTarget::TreeRange { .. } => ("tree range", "warn"),
        ReviewTarget::CommitRange { .. } => ("commit range", "mauve"),
    }
}

/// The `ReviewTarget` enum variant's own name, for the Request band's
/// "ReviewTarget::Variant" header -- distinct from [`anchor_kind`]'s
/// collapsed six-kind label, which the nav list badges use instead.
const fn target_variant_name(target: &ReviewTarget) -> &'static str {
    match target {
        ReviewTarget::Blobs(_) => "Blobs",
        ReviewTarget::Trees(_) => "Trees",
        ReviewTarget::Commits(_) => "Commits",
        ReviewTarget::TreeRange { .. } => "TreeRange",
        ReviewTarget::CommitRange { .. } => "CommitRange",
    }
}

/// Whether `hex` names an object that actually exists in `repo` -- the one
/// real signal available for a `ReviewTarget`: it is a bare oid (or pair of
/// oids), not a captured `gix-anchor` identity subtree, so there is no
/// stored anchor id or "captured at" commit to report, and no oracle to ask
/// for a rebound projection (see `anchor_projection`'s doc comment). Clean
/// vs. orphaned, computed by trying to resolve the object, is the honest
/// scope for a bare-oid target.
fn object_exists(repo: &gix::Repository, hex: &str) -> bool {
    gix::ObjectId::from_hex(hex.as_bytes())
        .ok()
        .is_some_and(|id| repo.try_find_object(id).ok().flatten().is_some())
}

/// `(clean, message)` for a target's projection onto the repository as it
/// exists right now.
///
/// `gix-anchor`'s oracles (`op_log` / `diff_trace` / `fingerprint`) resolve a
/// captured `Anchor`/`Span` -- the identity subtree behind a [`Comment`]'s
/// `Binding::Position` -- onto a moving target, and can report a *rebound*
/// (moved but still present) state as well as bound/orphaned. A
/// `ReviewTarget` is not one of those: it is a bare oid (or oid pair) with no
/// captured identity, so there is nothing for those oracles to run against.
/// The only honest check left is existence: does the object the review named
/// still resolve in this repository. That gives two real states -- clean and
/// orphaned -- not three; a "rebound" review-target state is not implemented
/// here because there is no cheap, real way to detect it for a bare oid.
fn anchor_projection(repo: &gix::Repository, target: &ReviewTarget) -> (bool, String) {
    match target {
        ReviewTarget::Blobs(items) => items.first().map_or_else(
            || (false, "empty target".into()),
            |item| single_projection(repo, &item.oid.to_string()),
        ),
        ReviewTarget::Trees(items) => items.first().map_or_else(
            || (false, "empty target".into()),
            |item| single_projection(repo, &item.oid.to_string()),
        ),
        ReviewTarget::Commits(items) => items.first().map_or_else(
            || (false, "empty target".into()),
            |oid| single_projection(repo, &oid.to_string()),
        ),
        ReviewTarget::TreeRange { base, tip } => {
            range_projection(repo, &base.oid.to_string(), &tip.oid.to_string())
        }
        ReviewTarget::CommitRange { base, tip } => {
            range_projection(repo, &base.to_string(), &tip.to_string())
        }
    }
}

fn single_projection(repo: &gix::Repository, oid: &str) -> (bool, String) {
    if object_exists(repo, oid) {
        (true, format!("projects cleanly onto {oid}"))
    } else {
        (
            false,
            format!("{oid} was not found in this repository -- this review's target is orphaned"),
        )
    }
}

fn range_projection(repo: &gix::Repository, a: &str, b: &str) -> (bool, String) {
    let (a_ok, b_ok) = (object_exists(repo, a), object_exists(repo, b));
    if a_ok && b_ok {
        (
            true,
            format!("both endpoints resolve -- projects cleanly onto {a}..{b}"),
        )
    } else {
        let missing = match (a_ok, b_ok) {
            (false, false) => format!("{a} and {b}"),
            (false, true) => a.to_owned(),
            (true, false) => b.to_owned(),
            (true, true) => unreachable!("handled by the a_ok && b_ok branch above"),
        };
        (
            false,
            format!(
                "{missing} not found in this repository -- this review's target range is orphaned"
            ),
        )
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
                    <label for="review-body">"Summary " <span class="muted">"(optional)"</span></label>
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
                    <label for="review-reviewers">"Reviewers " <span class="muted">"(comma-separated)"</span></label>
                    <input id="review-reviewers" name="reviewers" value=(form.reviewers.as_str()) placeholder="handle or member ID">
                    <label for="review-requesters">"Requesters " <span class="muted">"(comma-separated)"</span></label>
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

/// One master-list row's worth of real data: the review plus its comment
/// count and its signed-decision count (active and revoked together, same
/// total the Decisions band shows).
type ReviewRow = (Review, usize, usize);

async fn review_list(cx: &Cx, items: &[ReviewRow], selected: Option<&str>) -> Result {
    let can_create = can_create::<Review>(cx).await;
    view! { cx =>
        <div class="route-nav-header">
            <h2>"Reviews"</h2>
            if can_create {
                <a class="route-nav-action" href="/reviews/new">"+ request"</a>
            }
        </div>
        if items.is_empty() {
            <div class="empty-panel">
                <p>"No reviews found."</p>
                if can_create {
                    <a class="button-link" href="/reviews/new">"Request a review"</a>
                } else {
                    <p class="form-help">"Authenticate as a forge member to request a review."</p>
                }
            </div>
        } else {
            for (review, comment_count, decision_count) in items {
                let (kind_label, kind_class) = anchor_kind(&review.target);
                let edge_token = if review.status == "open" { "ok" } else { "faint" };
                let author = review.requesters.first().map(String::as_str).unwrap_or("unassigned");
                let decisions_label = match decision_count {
                    0 => "no decisions".to_owned(),
                    1 => "1 decision".to_owned(),
                    n => format!("{n} decisions"),
                };
                let is_selected = selected == Some(review.id.as_str());
                let row_class = if is_selected { "nav-row active" } else { "nav-row" };
                let edge_style = (!is_selected).then(|| format!("--nav-edge:var(--{edge_token})"));
                <a class=(row_class) style=(edge_style.unwrap_or_default()) href=(format!("/reviews/{}", review.id))>
                    <div class="nav-row-top">
                        <span class="nav-row-dot"></span>
                        <span class="nav-row-title">"Review " (review.id.as_str())</span>
                        <span class=(format!("anchor-kind {kind_class}")) style="margin-left:auto">(kind_label)</span>
                    </div>
                    <span class="nav-row-sub">(author) " · " (decisions_label) " · " (*comment_count) " comments"</span>
                </a>
            }
        }
    }
}

/// Attach comment and decision counts the same way the detail view needs
/// them, for the nav pane both routes share.
fn load_review_rows(
    repo: &gix::Repository,
) -> std::result::Result<Vec<ReviewRow>, gix_forge::Error> {
    let ids = Review::list(repo)?;
    ids.into_iter()
        .map(|id| Review::load_from_repo(repo, &id))
        .collect::<std::result::Result<Vec<_>, _>>()
        .map(|loaded| {
            loaded
                .into_iter()
                .flatten()
                .map(|review| {
                    let comment_count = review.get_comments(repo).map(|c| c.len()).unwrap_or(0);
                    let decision_count = ReviewDecision::claims_for_review(repo, &review.id)
                        .map(|claims| claims.len())
                        .unwrap_or(0);
                    (review, comment_count, decision_count)
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

#[page("/reviews")]
async fn reviews(cx: &Cx) -> Result {
    let data = with_repo(cx, load_review_rows).await;

    let (list, detail) = match data {
        Ok(reviews) => (
            review_list(cx, &reviews, None).await?,
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

    view! { route_shell(active: Tab::Reviews, title: "Reviews", nav_width: "21rem", nav: list, detail: detail) }
}

#[page("/reviews/new")]
async fn review_new(cx: &Cx) -> Result {
    if !can_create::<Review>(cx).await {
        let content = (view! { <div class="empty-panel"><h2>"Sign in to request a review"</h2><p class="muted">"Reads stay available without an account; creating a review requires a forge member identity."</p><a class="button-link secondary" href="/reviews">"Back to reviews"</a></div> })?;
        return view! { shell(active: Some(Tab::Reviews), title: "New review", keyword: None, child: content) };
    }
    let form = ReviewForm {
        body: String::new(),
        status: "open".to_owned(),
        reviewers: String::new(),
        requesters: String::new(),
        target: String::new(),
    };
    let content = review_form(cx, "/reviews", &form, None).await?;
    view! { shell(active: Some(Tab::Reviews), title: "New review", keyword: None, child: content) }
}

#[page(POST "/reviews")]
async fn review_create(cx: &Cx, Form(input): Form<HashMap<String, String>>) -> Result {
    let authorization = authorization(cx).await?;
    let form = ReviewForm::from_input(&input);
    if form.target.trim().is_empty() || ReviewTarget::parse(form.target.trim()).is_err() {
        let content = review_form(
            cx,
            "/reviews",
            &form,
            Some("Enter a valid review target using one of the supported target formats."),
        )
        .await?;
        return view! { shell(active: Some(Tab::Reviews), title: "New review", keyword: None, child: content) };
    }
    if Status::parse(&form.status).is_none() {
        let content = review_form(
            cx,
            "/reviews",
            &form,
            Some("Choose either Open or Closed for the status."),
        )
        .await?;
        return view! { shell(active: Some(Tab::Reviews), title: "New review", keyword: None, child: content) };
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
    let result = with_repo(cx, move |repo| {
        review.create_in_repo_as(repo, &authorization)
    })
    .await;
    match result {
        Ok(id) => {
            let content = (view! { <section class="success-panel"><span class="success-icon">"✓"</span><p class="eyebrow">"REVIEW CREATED"</p><h2>"Your review request is ready"</h2><p class="muted">"Reviewers can now find the target and leave feedback."</p><a class="button-link" href=(format!("/reviews/{id}"))>"View review"</a></section> })?;
            view! { shell(active: Some(Tab::Reviews), title: "Review created", keyword: None, child: content) }
        }
        Err(error) => {
            if let Some(error) = authorization_error(&error) {
                return Err(error);
            }
            let content = review_form(cx, "/reviews", &form, Some(&error)).await?;
            view! { shell(active: Some(Tab::Reviews), title: "New review", keyword: None, child: content) }
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
                let (anchor_clean, anchor_message) = anchor_projection(repo, &review.target);
                let comment_rows: Vec<(Comment, Option<String>)> = comments
                    .iter()
                    .map(|comment| {
                        let key = signing_key_of(repo, &comment.author);
                        (comment.clone(), key)
                    })
                    .collect();
                let decision_rows = ReviewDecision::claims_for_review(repo, &review.id)?;
                let review_items = load_review_rows(repo)?;
                Ok(Some((
                    review_items,
                    review,
                    comment_rows,
                    decision_rows,
                    anchor_clean,
                    anchor_message,
                )))
            }
            None => Ok(None),
        }
    })
    .await;

    let (list, detail) = match data {
        Ok(Some((
            review_items,
            review,
            comment_rows,
            decision_rows,
            anchor_clean,
            anchor_message,
        ))) => {
            let can_edit = can_update(cx, &review).await;
            let can_comment = can_create::<Comment>(cx).await;
            let can_decide = can_create::<Review>(cx).await;
            let next_status = if review.status == "open" {
                "closed"
            } else {
                "open"
            };
            let toggle_label = if next_status == "closed" {
                "Close review"
            } else {
                "Reopen review"
            };
            let status_class = if review.status == "open" {
                "status ok"
            } else {
                "status neutral"
            };
            let author = review
                .requesters
                .first()
                .map(String::as_str)
                .unwrap_or("unassigned");
            let reviewers_label = if review.reviewers.is_empty() {
                "none invited".to_owned()
            } else {
                review.reviewers.join(", ")
            };
            let target_shorthand = review.target.to_string();
            let target_variant = target_variant_name(&review.target);
            // Non-negotiable #8: status is a plain manual field the owner
            // sets by hand -- this only *suggests* they might want to, never
            // computes or sets it itself.
            let has_active_decision = decision_rows
                .iter()
                .any(|decision| decision.revoked_by.is_none());
            let show_nudge = can_edit && review.status == "open" && has_active_decision;
            // Non-negotiable #4: once the pinned oid no longer resolves, the
            // decisions signed against it are shown as quiet muted lines
            // rather than hidden or specially flagged.
            let verdict_list_class = if anchor_clean {
                "verdict-list"
            } else {
                "verdict-list stale"
            };
            (
                review_list(cx, &review_items, Some(review.id.as_str())).await?,
                (view! { cx =>
                    <article class="route-main-width-lg">
                        <h1 class="review-title">"Review " (review.id.as_str())</h1>
                        <p class="review-subtitle">(review.id.as_str()) " · requested by " (author)</p>

                        if can_edit {
                            <div class="detail-actions">
                                <a class="button-link secondary" href=(format!("/reviews/{}/edit", review.id))>"Edit"</a>
                                <form class="inline-form" action=(format!("/reviews/{}/delete", review.id)) method="post"><button class="danger-button" type="submit">"Delete"</button></form>
                            </div>
                        } else {
                            <p class="action-note">"Only the requester or a maintainer can change this review."</p>
                        }

                        <h3 class="step-heading"><span class="step-badge">"1"</span>"Request"</h3>
                        <div class="body-copy rendered-file">(Unescaped::new_unchecked(render_asciidoc(&review.body)))</div>
                        <div class="panel-block target-box">
                            <div class="panel-block-heading target-box-head">
                                <span>"ReviewTarget::" (target_variant)</span>
                                <span class="target-box-note">"exact oid — resolves or is orphaned, never drifts"</span>
                            </div>
                            <div class="panel-block-body">
                                <div class="target-cmd">
                                    <span class="target-cmd-prompt">"$"</span>
                                    <code>"gf review show " <span class="target-cmd-value">(target_shorthand.as_str())</span></code>
                                </div>
                                <p class="target-explain">(anchor_message.as_str())</p>
                                <div class="target-parties">
                                    <span><strong>"requester"</strong> " — " (author) " (owns this record)"</span>
                                    <span><strong>"reviewers"</strong> " — " (reviewers_label.as_str()) " (invited, no powers)"</span>
                                </div>
                            </div>
                        </div>

                        <h3 class="step-heading"><span class="step-badge">"2"</span>"Discussion"</h3>
                        <section class="comments">
                            <div class="panel-heading"><div><span class="muted">(comment_rows.len()) " comments, scoped under (\"review\", " (review.id.as_str()) ")"</span></div> if can_comment { <a class="button-link secondary" href="/comments/new">"New comment"</a> }</div>
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
                                <form class="comment-form" action=(format!("/reviews/{}/comments", review.id)) method="post">
                                    <label for="review-comment-author">"Add a comment"</label>
                                    <input id="review-comment-author" name="author" value="authenticated member" type="hidden">
                                    <textarea name="body" rows="5" placeholder="Leave a helpful update or question." required=""></textarea>
                                    <button type="submit">"Comment"</button>
                                </form>
                            } else {
                                <p class="form-context">"Sign in as a forge member to join this discussion."</p>
                            }
                        </section>

                        <h3 class="step-heading"><span class="step-badge">"3"</span>"Decisions"</h3>
                        if show_nudge {
                            <div class="decision-nudge">
                                <span>"This review already has a signed decision on record."</span>
                                <form action=(format!("/reviews/{}/status", review.id)) method="post">
                                    <input type="hidden" name="status" value="closed">
                                    <button type="submit">"Close review"</button>
                                </form>
                            </div>
                        }
                        <div class="panel-block">
                            <div class="panel-block-heading">"Signed decisions"</div>
                            <p class="verdicts-note">
                                "Decisions are signed claims about this review target. A revoked decision remains visible in the chain and is marked below."
                                if !anchor_clean { " The pinned target no longer resolves, so every decision below is shown muted." }
                            </p>
                            if decision_rows.is_empty() {
                                <p class="empty panel-block-body">"No signed decisions yet."</p>
                            } else {
                                <ul class=(verdict_list_class)>
                                    for decision in &decision_rows {
                                        <li class="verdict-row">
                                            <span>
                                                if decision.decision.verdict == ReviewVerdict::Approve { "Approve" } else { "Deny" }
                                                if let Some(message) = &decision.decision.message { <span class="muted">" — " (message.as_str())</span> }
                                            </span>
                                            <span class=(if decision.revoked_by.is_some() { "verdict-pill pending" } else if decision.decision.verdict == ReviewVerdict::Approve { "verdict-pill responded" } else { "verdict-pill denied" })>
                                                if decision.revoked_by.is_some() { "revoked" } else { "active" }
                                            </span>
                                            <small class="claim-chain">
                                                "claim " <code>(decision.claim_id.to_string())</code> " · key " <code>(decision.key_claim_id.to_string())</code>
                                                if let Some(revocation) = decision.revoked_by { " · revoked by " <code>(revocation.to_string())</code> }
                                            </small>
                                        </li>
                                    }
                                </ul>
                            }
                            if can_decide {
                                <p class="form-help"><a href=(format!("/reviews/{}/decision", review.id))>"Sign with a written message instead"</a></p>
                            }
                        </div>

                        <h3 class="step-heading"><span class="step-badge">"4"</span>"Status"</h3>
                        <div class="status-band">
                            <span class=(status_class)>(review.status.as_str())</span>
                            if can_edit {
                                <form class="inline-form" action=(format!("/reviews/{}/status", review.id)) method="post">
                                    <input type="hidden" name="status" value=(next_status)>
                                    <button type="submit">(toggle_label)</button>
                                </form>
                            } else {
                                <span class="action-note">"Set by the requester or a maintainer."</span>
                            }
                            <span class="status-band-note">"A plain toggle the requester or a maintainer sets by hand — nothing here computes it from the decisions above."</span>
                        </div>

                        if can_decide {
                            <div class="final-actions">
                                <form action=(format!("/reviews/{}/decision", review.id)) method="post">
                                    <input type="hidden" name="verdict" value="approve">
                                    <button type="submit">"Sign approval"</button>
                                </form>
                                <form action=(format!("/reviews/{}/decision", review.id)) method="post">
                                    <input type="hidden" name="verdict" value="deny">
                                    <button class="button-secondary" type="submit">"Sign denial"</button>
                                </form>
                            </div>
                        }
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

    view! { route_shell(active: Tab::Reviews, title: "Review", nav_width: "21rem", nav: list, detail: detail) }
}

#[page("/reviews/{review_id}/decision")]
async fn review_decision_page(cx: &Cx) -> Result {
    let id = path_param::<ReviewId>(cx)?.clone();
    let content = (view! { cx =>
        <section class="form-layout">
            <div class="form-intro">
                <p class="eyebrow">"SIGNED REVIEW DECISION"</p>
                <h2>"Record your decision"</h2>
                <p class="muted">"Your approval or denial will be signed and appended to this review target's claim chain."</p>
            </div>
            <form class="entity-form" action=(format!("/reviews/{id}/decision")) method="post">
                <fieldset>
                    <legend>"Decision"</legend>
                    <label for="decision-message">"Message " <span class="muted">"(optional)"</span></label>
                    <textarea id="decision-message" name="message" rows="7" placeholder="Explain your decision."></textarea>
                </fieldset>
                <div class="form-actions">
                    <a class="button-link secondary" href=(format!("/reviews/{id}"))>"Cancel"</a>
                    <button type="submit" name="verdict" value="approve">"Approve"</button>
                    <button class="danger-button" type="submit" name="verdict" value="deny">"Deny"</button>
                </div>
            </form>
        </section>
    })?;
    view! { shell(active: Some(Tab::Reviews), title: "Record decision", keyword: None, child: content) }
}

#[page(POST "/reviews/{review_id}/decision")]
async fn review_decision_submit(cx: &Cx, Form(input): Form<HashMap<String, String>>) -> Result {
    let authorization = authorization(cx).await?;
    let id = path_param::<ReviewId>(cx)?.clone();
    let verdict = input.get("verdict").map(String::as_str).unwrap_or_default();
    let message = input
        .get("message")
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_owned);
    let Some(verdict) = (match verdict {
        "approve" => Some(ReviewVerdict::Approve),
        "deny" => Some(ReviewVerdict::Deny),
        _ => None,
    }) else {
        return view! { shell(active: Some(Tab::Reviews), title: "Decision failed", keyword: None, child: (view! { <div class="error-panel"><strong>"Choose Approve or Deny"</strong><a href=(format!("/reviews/{id}/decision"))>"Return to decision form"</a></div> })?) };
    };
    if let Some(confirm) = confirm_step(
        cx,
        &input,
        &format!("/reviews/{id}/decision"),
        &format!("refs/meta/review/{id}"),
        &format!("/reviews/{id}"),
        (view! { cx => "Sign a " <strong>(if verdict == ReviewVerdict::Approve { "approval" } else { "denial" })</strong> " for review " <code>(id.as_str())</code> })?,
    ).await? {
        return view! { shell(active: Some(Tab::Reviews), title: "Confirm decision", keyword: None, child: confirm) };
    }
    let lookup_id = id.clone();
    let result = with_repo(cx, move |repo| match verdict {
        ReviewVerdict::Approve => {
            ReviewDecision::approve_as(repo, &authorization, lookup_id, message)
        }
        ReviewVerdict::Deny => ReviewDecision::deny_as(repo, &authorization, lookup_id, message),
    })
    .await;
    match result {
        Ok(claim_id) => {
            let content = (view! { <section class="success-panel"><span class="success-icon">"✓"</span><p class="eyebrow">"SIGNED DECISION"</p><h2>"Decision recorded"</h2><p class="muted">"Claim " <code>(claim_id.to_string())</code> " was appended to the review chain."</p><a class="button-link" href=(format!("/reviews/{id}"))>"Return to review"</a></section> })?;
            view! { shell(active: Some(Tab::Reviews), title: "Decision recorded", keyword: None, child: content) }
        }
        Err(error) => {
            if let Some(error) = authorization_error(&error) {
                return Err(error);
            }
            let content = (view! { <div class="error-panel"><strong>"Decision could not be signed"</strong><p>(error)</p><a href=(format!("/reviews/{id}/decision"))>"Return to decision form"</a></div> })?;
            view! { shell(active: Some(Tab::Reviews), title: "Decision failed", keyword: None, child: content) }
        }
    }
}

#[page("/reviews/{review_id}/edit")]
async fn review_edit_page(cx: &Cx) -> Result {
    let id = path_param::<ReviewId>(cx)?.clone();
    let data = with_repo(cx, move |repo| Review::load_from_repo(repo, &id)).await;
    let content = match data {
        Ok(Some(review)) => {
            if can_update(cx, &review).await {
                review_form(
                    cx,
                    &format!("/reviews/{}/edit", review.id),
                    &ReviewForm::from_review(&review),
                    None,
                )
                .await?
            } else {
                (view! { <div class="empty-panel"><h2>"This review is read-only for you"</h2><p class="muted">"Only the requester or a maintainer can edit or delete it."</p><a class="button-link secondary" href=(format!("/reviews/{}", review.id))>"Back to review"</a></div> })?
            }
        }
        Ok(None) => {
            (view! { <div class="empty-panel"><h2>"Review not found"</h2><a href="/reviews">"Back to reviews"</a></div> })?
        }
        Err(error) => {
            (view! { <div class="error-panel"><strong>"Could not load review"</strong><p>(error)</p></div> })?
        }
    };
    view! { shell(active: Some(Tab::Reviews), title: "Edit review", keyword: None, child: content) }
}

#[page(POST "/reviews/{review_id}/edit")]
async fn review_edit_submit(cx: &Cx, Form(input): Form<HashMap<String, String>>) -> Result {
    let authorization = authorization(cx).await?;
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
        return view! { shell(active: Some(Tab::Reviews), title: "Edit review", keyword: None, child: content) };
    }
    let lookup_id = id.clone();
    let result = with_repo(cx, move |repo| {
        let mut review = Review::load_from_repo(repo, &lookup_id)?.ok_or_else(|| {
            gix_forge::Error::InvalidTarget(format!("review `{lookup_id}` was not found"))
        })?;
        review.body = form.body.trim().to_owned();
        review.status = form.status.trim().to_ascii_lowercase();
        review.reviewers = csv_values(&form.reviewers);
        review.target = ReviewTarget::parse(form.target.trim()).expect("validated review target");
        review
            .save_in_repo_as(repo, &authorization)
            .map(|_| review.id)
    })
    .await;
    match result {
        Ok(id) => {
            let content = (view! { <section class="success-panel"><span class="success-icon">"✓"</span><p class="eyebrow">"SAVED"</p><h2>"Review updated"</h2><a class="button-link" href=(format!("/reviews/{id}"))>"View review"</a></section> })?;
            view! { shell(active: Some(Tab::Reviews), title: "Review updated", keyword: None, child: content) }
        }
        Err(error) => {
            if let Some(error) = authorization_error(&error) {
                return Err(error);
            }
            let content = (view! { <div class="error-panel"><strong>"Review could not be saved"</strong><p>(error)</p><a href=(format!("/reviews/{id}/edit"))>"Return to editor"</a></div> })?;
            view! { shell(active: Some(Tab::Reviews), title: "Edit review", keyword: None, child: content) }
        }
    }
}

#[page(POST "/reviews/{review_id}/status")]
async fn review_status(cx: &Cx, Form(input): Form<HashMap<String, String>>) -> Result {
    let authorization = authorization(cx).await?;
    let id = path_param::<ReviewId>(cx)?.clone();
    let status = input
        .get("status")
        .map(String::as_str)
        .unwrap_or_default()
        .to_owned();
    if let Some(confirm) = confirm_step(
        cx,
        &input,
        &format!("/reviews/{id}/status"),
        &format!("refs/meta/review/{id}"),
        &format!("/reviews/{id}"),
        (view! { cx => "Change review " <code>(id.as_str())</code> " status to " <code>(status.as_str())</code> })?,
    )
    .await?
    {
        return view! { shell(active: Some(Tab::Reviews), title: "Confirm status change", keyword: None, child: confirm) };
    }
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
        review
            .save_in_repo_as(repo, &authorization)
            .map(|_| review.id)
    })
    .await;
    match result {
        Ok(id) => {
            let content = (view! { <section class="success-panel"><span class="success-icon">"✓"</span><h2>"Review status updated"</h2><a class="button-link" href=(format!("/reviews/{id}"))>"Return to review"</a></section> })?;
            view! { shell(active: Some(Tab::Reviews), title: "Review updated", keyword: None, child: content) }
        }
        Err(error) => {
            if let Some(error) = authorization_error(&error) {
                return Err(error);
            }
            let content = (view! { <div class="error-panel"><strong>"Could not update review"</strong><p>(error)</p><a href=(format!("/reviews/{id}"))>"Return to review"</a></div> })?;
            view! { shell(active: Some(Tab::Reviews), title: "Review action failed", keyword: None, child: content) }
        }
    }
}

#[page(POST "/reviews/{review_id}/comments")]
async fn review_comment(cx: &Cx, Form(input): Form<HashMap<String, String>>) -> Result {
    let authorization = authorization(cx).await?;
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
        let content = (view! { <div class="error-panel"><strong>"Comment could not be saved"</strong><p>"Your name and a comment are required."</p><a href=(format!("/reviews/{id}"))>"Return to review"</a></div> })?;
        return view! { shell(active: Some(Tab::Reviews), title: "Comment failed", keyword: None, child: content) };
    }
    if let Some(confirm) = confirm_step(
        cx,
        &input,
        &format!("/reviews/{id}/comments"),
        &format!("refs/meta/review/{id}"),
        &format!("/reviews/{id}"),
        (view! { cx => "Post a new comment on review " <code>(id.as_str())</code> })?,
    )
    .await?
    {
        return view! { shell(active: Some(Tab::Reviews), title: "Confirm comment", keyword: None, child: confirm) };
    }
    let lookup_id = id.clone();
    let result = with_repo(cx, move |repo| {
        let review = Review::load_from_repo(repo, &lookup_id)?.ok_or_else(|| {
            gix_forge::Error::InvalidTarget(format!("review `{lookup_id}` was not found"))
        })?;
        review.add_comment_as(repo, &authorization, &body)
    })
    .await;
    match result {
        Ok(_) => {
            let content = (view! { <section class="success-panel"><span class="success-icon">"✓"</span><h2>"Comment added"</h2><a class="button-link" href=(format!("/reviews/{id}"))>"Return to review"</a></section> })?;
            view! { shell(active: Some(Tab::Reviews), title: "Comment added", keyword: None, child: content) }
        }
        Err(error) => {
            if let Some(error) = authorization_error(&error) {
                return Err(error);
            }
            let content = (view! { <div class="error-panel"><strong>"Comment could not be saved"</strong><p>(error)</p><a href=(format!("/reviews/{id}"))>"Return to review"</a></div> })?;
            view! { shell(active: Some(Tab::Reviews), title: "Comment failed", keyword: None, child: content) }
        }
    }
}

#[page(POST "/reviews/{review_id}/delete")]
async fn review_delete(cx: &Cx) -> Result {
    let authorization = authorization(cx).await?;
    let id = path_param::<ReviewId>(cx)?.clone();
    let lookup_id = id.clone();
    let result = with_repo(cx, move |repo| {
        Review::delete_as(repo, &lookup_id, &authorization)
    })
    .await;
    let content = match result {
        Ok(true) => {
            (view! { <section class="success-panel"><span class="success-icon">"✓"</span><h2>"Review deleted"</h2><a class="button-link" href="/reviews">"Return to reviews"</a></section> })?
        }
        Ok(false) => {
            (view! { <div class="empty-panel"><h2>"Review not found"</h2><a href="/reviews">"Return to reviews"</a></div> })?
        }
        Err(error) => {
            if let Some(error) = authorization_error(&error) {
                return Err(error);
            }
            (view! { <div class="error-panel"><strong>"Review could not be deleted"</strong><p>(error)</p><a href=(format!("/reviews/{id}"))>"Return to review"</a></div> })?
        }
    };
    view! { shell(active: Some(Tab::Reviews), title: "Delete review", keyword: None, child: content) }
}
