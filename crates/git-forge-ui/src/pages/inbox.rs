//! The Inbox route (Handoff v5): `GET /inbox`, the app's landing page.
//! The Inbox route, grouped by real lifecycle state. Said is intentionally
//! absent: the forge store has no notification directionality, so it cannot
//! honestly answer whether a reply was addressed to the current user.

use gix_forge::{
    AnnotationState, Comment, EntityOps, Issue, Review, ReviewDecision, ReviewDecisionClaim,
    ReviewVerdict,
};
use topcoat::{Result, context::Cx, router::page, view::view};

use crate::{
    pages::with_repo,
    shell::{Tab, full_shell},
};

/// One row's worth of data for any inbox group -- real or sample.
struct InboxItem {
    title: String,
    sub: String,
    /// The actual forge ref this row's entity lives at (real rows), or a
    /// representative sample path (sample rows) -- see module doc comment.
    reference: String,
    /// The store has no per-entity timestamp, so real rows omit age.
    age: Option<&'static str>,
    action: &'static str,
    href: String,
}

/// One inbox group. `slug` supplies this group's semantic color.
struct InboxGroup {
    slug: &'static str,
    label: &'static str,
    items: Vec<InboxItem>,
    fix_all_href: Option<&'static str>,
}

/// Real data pulled from the repository for every displayed group.
struct InboxRepoData {
    asked: Vec<Issue>,
    decided: Vec<(Review, ReviewDecisionClaim)>,
    blocked: Vec<Review>,
    drifted: Vec<Comment>,
}

fn load_inbox_data(repo: &gix::Repository) -> std::result::Result<InboxRepoData, gix_forge::Error> {
    let issue_ids = Issue::list(repo)?;
    let asked = issue_ids
        .into_iter()
        .filter_map(|id| Issue::load_from_repo(repo, &id).ok().flatten())
        .filter(|issue| issue.status == "open")
        .collect();

    let review_ids = Review::list(repo)?;
    let reviews: Vec<Review> = review_ids
        .into_iter()
        .filter_map(|id| Review::load_from_repo(repo, &id).ok().flatten())
        .collect();

    let mut decided = Vec::new();
    let mut blocked = Vec::new();
    let head = repo.head_id().ok().map(|id| id.detach());
    let drifted = Comment::list(repo)?
        .into_iter()
        .filter_map(|id| Comment::load_from_repo(repo, &id).ok().flatten())
        .filter(|comment| {
            head.is_some_and(|head| {
                matches!(
                    Comment::annotation_state(repo, comment, head),
                    Ok(Some(AnnotationState::Drifted))
                )
            })
        })
        .collect();
    for review in reviews {
        // Best-effort: a review whose target no longer resolves can make
        // the attest layer error; that review is neither decided nor
        // countable as blocked-on-a-decision, so it is simply skipped.
        let claims = ReviewDecision::claims_for_review(repo, &review.id).unwrap_or_default();
        let active = claims.into_iter().find(|claim| claim.revoked_by.is_none());
        match (review.status.as_str(), active) {
            (_, Some(claim)) => decided.push((review, claim)),
            ("open", None) => blocked.push(review),
            _ => {}
        }
    }

    Ok(InboxRepoData {
        asked,
        decided,
        blocked,
        drifted,
    })
}

fn asked_items(issues: Vec<Issue>) -> Vec<InboxItem> {
    issues
        .into_iter()
        .map(|issue| {
            let reporter = issue
                .reporters
                .first()
                .cloned()
                .unwrap_or_else(|| "unassigned".to_owned());
            InboxItem {
                title: issue.title.clone(),
                sub: format!("opened by {reporter}"),
                reference: format!("refs/forge/issue/{}", issue.id),
                age: None,
                action: "reply",
                href: format!("/issues/{}", issue.id),
            }
        })
        .collect()
}

fn decided_items(rows: Vec<(Review, ReviewDecisionClaim)>) -> Vec<InboxItem> {
    rows.into_iter()
        .map(|(review, claim)| {
            let verdict = if claim.decision.verdict == ReviewVerdict::Approve {
                "approved"
            } else {
                "denied"
            };
            let sub = match &claim.decision.message {
                Some(message) => format!("{verdict} — {message}"),
                None => verdict.to_owned(),
            };
            InboxItem {
                title: format!("Review {}", review.id),
                sub,
                reference: format!("refs/forge/review/{}", review.id),
                age: None,
                action: "review",
                href: format!("/reviews/{}", review.id),
            }
        })
        .collect()
}

fn blocked_items(reviews: Vec<Review>) -> Vec<InboxItem> {
    reviews
        .into_iter()
        .map(|review| {
            let requester = review
                .requesters
                .first()
                .cloned()
                .unwrap_or_else(|| "unassigned".to_owned());
            InboxItem {
                title: format!("Review {}", review.id),
                sub: format!("requested by {requester} · awaiting a decision"),
                reference: format!("refs/forge/review/{}", review.id),
                age: None,
                action: "review",
                href: format!("/reviews/{}", review.id),
            }
        })
        .collect()
}

fn drifted_items(comments: Vec<Comment>) -> Vec<InboxItem> {
    comments
        .into_iter()
        .map(|comment| InboxItem {
            title: comment
                .body
                .lines()
                .next()
                .unwrap_or("Drifted annotation")
                .to_owned(),
            sub: "binding resolved away from its captured anchor".to_owned(),
            reference: format!("refs/forge/comment/{}", comment.id),
            age: None,
            action: "repair",
            href: format!("/annotations/{}?repair=1", comment.id),
        })
        .collect()
}

async fn inbox_group_section(cx: &Cx, group: &InboxGroup) -> Result {
    view! { cx =>
        if !group.items.is_empty() {
            <section class="inbox-group">
                <h2 class=(format!("inbox-group-heading {}", group.slug))>
                    <span class="inbox-group-dot"></span>
                    (group.label)
                    <span class="inbox-group-count">(group.items.len())</span>
                    if let Some(href) = group.fix_all_href {
                        <a class="inbox-fix-all" href=(href)>"fix all " (group.items.len()) " →"</a>
                    }
                </h2>
                <div class="inbox-rows">
                    for item in &group.items {
                        <a class="inbox-row" href=(item.href.as_str())>
                            <span class="inbox-row-main">
                                <span class="inbox-row-title-line">
                                    <span class="inbox-row-title">(item.title.as_str())</span>
                                    if let Some(age) = item.age {
                                        <span class="inbox-row-age">(age)</span>
                                    }
                                </span>
                                <span class="inbox-row-sub">(item.sub.as_str())</span>
                                <span class="inbox-row-ref">(item.reference.as_str())</span>
                            </span>
                            <span class=(format!("inbox-row-action {}", group.slug))>(item.action)</span>
                        </a>
                    }
                </div>
            </section>
        }
    }
}

#[page("/inbox")]
async fn inbox(cx: &Cx) -> Result {
    let data = with_repo(cx, load_inbox_data).await;

    let content = match data {
        Ok(data) => {
            let groups = [
                InboxGroup {
                    slug: "asked",
                    label: "Asked",
                    items: asked_items(data.asked),
                    fix_all_href: None,
                },
                InboxGroup {
                    slug: "decided",
                    label: "Decided",
                    items: decided_items(data.decided),
                    fix_all_href: None,
                },
                InboxGroup {
                    slug: "drifted",
                    label: "Drifted",
                    items: drifted_items(data.drifted),
                    fix_all_href: Some("/annotations"),
                },
                InboxGroup {
                    slug: "blocked",
                    label: "Blocked",
                    items: blocked_items(data.blocked),
                    fix_all_href: None,
                },
            ];
            let total: usize = groups.iter().map(|group| group.items.len()).sum();

            (view! { cx =>
                <div class="inbox">
                    <div class="inbox-header">
                        <h1>"Inbox"</h1>
                        <span class="inbox-meta">(total) " items need you · last opened 3h ago"</span>
                    </div>
                    <p class="inbox-sub">"Grouped by which lifecycle moved — asked, decided, drifted, blocked. Not by urgency."</p>

                    for group in &groups {
                        (inbox_group_section(cx, group).await?)
                    }
                </div>
            })?
        }
        Err(error) => {
            (view! { cx =>
                <div class="inbox">
                    <div class="inbox-header"><h1>"Inbox"</h1></div>
                    <p class="error-text">(error)</p>
                </div>
            })?
        }
    };

    view! { full_shell(active: Tab::Inbox, title: "Inbox", child: content) }
}
