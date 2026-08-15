//! The Inbox route (Handoff v5): `GET /inbox`, the app's landing page.
//! Grouped by which lifecycle event moved -- Asked, Said, Decided, Drifted,
//! Blocked -- not by urgency, per the design mock.
//!
//! ## Real vs. sample data
//!
//! There is no notification/inbox backend in this codebase (no read/unread
//! state, no "while you were away" digest, no drift-reprojection engine --
//! see `gix-forge`'s query surface and `local.rs`'s doc comment for the same
//! observation about drift/gate state). Rather than leave every row as
//! sample content, this route wires up what is cheaply real from the
//! existing entity store, and keeps the rest as clearly-commented sample
//! content, matching the precedent set by `local.rs` and the former
//! `workingcopy.rs` board:
//!
//! - **Asked** is real: every open [`Issue`], approximated as "something a
//!   reporter opened that needs a reply." Neither `Issue` nor `Comment`
//!   stores a timestamp anywhere in this codebase (see `issues.rs`'s
//!   `signing_key_of` doc comment), so there is no honest "age" to show for
//!   these rows -- the age chip is simply omitted rather than fabricated.
//! - **Decided** is real: every [`Review`] with at least one active (i.e.
//!   not revoked) signed [`ReviewDecision`] claim, via
//!   `ReviewDecision::claims_for_review` -- the same signed-claim chain
//!   `reviews.rs`'s detail view reads. Claims carry no reviewer identity
//!   this UI can cheaply resolve to a name (only object ids), so the row
//!   shows the verdict and message, not a "who" -- consistent with how
//!   `reviews.rs` itself displays decisions.
//! - **Blocked** is real: every open `Review` with *no* active decision yet
//!   -- the review is waiting on a human, per the "blocked" semantic.
//! - **Said** ("replies to you") and **Drifted** (bindings that fell below
//!   the rule threshold) have no honest backend to approximate cheaply:
//!   "to you" requires notification directionality this store doesn't
//!   record, and binding re-projection/drift detection doesn't exist here
//!   yet (per `local.rs`). Both groups, and the "while you were away"
//!   digest banner, are sample content previewing the intended design --
//!   the one exception is the drift banner's numbers, which were chosen to
//!   agree with the sample Drifted row count below.

use gix_forge::{EntityOps, Issue, Review, ReviewDecision, ReviewDecisionClaim, ReviewVerdict};
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
    /// `None` when there is no honest age to report (every real row: this
    /// store keeps no per-entity timestamp -- see module doc comment).
    age: Option<&'static str>,
    action: &'static str,
    href: String,
}

/// One inbox group. `slug` doubles as the CSS modifier class that supplies
/// this group's semantic color (`assets/style.css`, "Inbox" section) --
/// Asked/Said share `--acc`, Decided is `--ok`, Drifted is `--warn`,
/// Blocked is `--bad`, per the design mock's non-negotiable color mapping.
struct InboxGroup {
    slug: &'static str,
    label: &'static str,
    items: Vec<InboxItem>,
    fix_all_href: Option<&'static str>,
}

/// Real data pulled from the repository for the Asked/Decided/Blocked
/// groups.
struct InboxRepoData {
    asked: Vec<Issue>,
    decided: Vec<(Review, ReviewDecisionClaim)>,
    blocked: Vec<Review>,
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

/// Sample content: there is no notification backend recording who replied
/// "to you" specifically -- see module doc comment.
fn sample_said_items() -> Vec<InboxItem> {
    vec![
        InboxItem {
            title: "Re: binding drift on gix-panic-fix".to_owned(),
            sub: "maria replied on your comment".to_owned(),
            reference: "refs/forge/comment/8f2c1a0".to_owned(),
            age: Some("2h"),
            action: "reply",
            href: "/issues".to_owned(),
        },
        InboxItem {
            title: "Re: review target orphaned after rebase".to_owned(),
            sub: "devon replied on your review comment".to_owned(),
            reference: "refs/forge/comment/3a91be4".to_owned(),
            age: Some("6h"),
            action: "reply",
            href: "/reviews".to_owned(),
        },
    ]
}

/// Sample content: no binding re-projection/drift engine exists yet -- see
/// module doc comment. The count here (3) matches the "while you were
/// away" banner's "3 fell below the rule threshold" figure.
fn sample_drifted_items() -> Vec<InboxItem> {
    vec![
        InboxItem {
            title: "src/engine/dispatch.rs:142".to_owned(),
            sub: "binding confidence 0.61, below the 0.80 rule threshold".to_owned(),
            reference: "refs/forge/comment/c771e02".to_owned(),
            age: Some("9h"),
            action: "repair",
            href: "/annotations".to_owned(),
        },
        InboxItem {
            title: "src/engine/panic.rs:58".to_owned(),
            sub: "binding confidence 0.74, below the 0.80 rule threshold".to_owned(),
            reference: "refs/forge/comment/9d4f710".to_owned(),
            age: Some("9h"),
            action: "repair",
            href: "/annotations".to_owned(),
        },
        InboxItem {
            title: "src/store/schema.rs:23".to_owned(),
            sub: "binding confidence 0.55, below the 0.80 rule threshold".to_owned(),
            reference: "refs/forge/comment/1b6a9f3".to_owned(),
            age: Some("9h"),
            action: "repair",
            href: "/annotations".to_owned(),
        },
    ]
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
                    slug: "said",
                    label: "Said",
                    items: sample_said_items(),
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
                    items: sample_drifted_items(),
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
                    <p class="inbox-sub">"Grouped by which lifecycle moved — asked, said, decided, drifted, blocked. Not by urgency."</p>

                    <div class="inbox-away">
                        <div class="inbox-away-head">
                            <strong>"While you were away"</strong>
                            <span class="inbox-away-sep">"·"</span>
                            <span class="inbox-away-detail">"you rebased " <span class="inbox-away-hl">"maria/gix-panic-fix"</span> " onto " <span class="inbox-away-hl">"main"</span> " (14 commits)"</span>
                        </div>
                        <p class="inbox-away-body body-copy">
                            <span class="inbox-away-ok">"11 bindings"</span> " re-projected cleanly and their comments moved with the code. "
                            <span class="inbox-away-warn">"3"</span> " fell below the rule threshold of 0.80 and are waiting on a human. One merge gate went red as a result."
                        </p>
                    </div>

                    for group in &groups {
                        (inbox_group_section(cx, group).await?)
                    }
                </div>
            })?
        }
        Err(error) => (view! { cx =>
            <div class="inbox">
                <div class="inbox-header"><h1>"Inbox"</h1></div>
                <p class="error-text">(error)</p>
            </div>
        })?,
    };

    view! { full_shell(active: Tab::Inbox, title: "Inbox", child: content) }
}
