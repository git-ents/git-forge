use gix_forge::query_goal;
use topcoat::{Result, context::Cx, router::page, view::view};

use crate::{
    pages::with_repo,
    shell::{Tab, shell},
};

#[page("/")]
async fn dashboard(cx: &Cx) -> Result {
    let data = with_repo(cx, |repo| {
        let issues = query_goal(repo, "issue_status(Id, Status)", &["Id", "Status"])?;
        let reviews = query_goal(repo, "review_status(Id, Status)", &["Id", "Status"])?;
        let mut activity = issues
            .iter()
            .rev()
            .take(3)
            .map(|row| format!("Issue {} · {}", row[0], row[1]))
            .collect::<Vec<_>>();
        activity.extend(
            reviews
                .iter()
                .rev()
                .take(3)
                .map(|row| format!("Review {} · {}", row[0], row[1])),
        );
        Ok((issues.len(), reviews.len(), activity))
    })
    .await;

    let content = match data {
        Ok((issue_count, review_count, activity)) => {
            (view! {
                <div class="stats-grid">
                    <a class="stat-card" href="/issues">
                        <span class="stat-label">"Issues"</span>
                        <strong>(issue_count)</strong>
                    </a>
                    <a class="stat-card" href="/reviews">
                        <span class="stat-label">"Reviews"</span>
                        <strong>(review_count)</strong>
                    </a>
                </div>
                <section class="panel">
                    <div class="panel-heading">
                        <h2>"Recent activity"</h2>
                        <span class="muted">"from forge status facts"</span>
                    </div>
                    if activity.is_empty() {
                        <p class="empty">"No issue or review activity yet."</p>
                    } else {
                        <ul class="activity-list">
                            for item in activity {
                                <li>(item)</li>
                            }
                        </ul>
                    }
                    <div class="form-actions">
                        <a class="button-link secondary" href="/comments/new">"Start a discussion"</a>
                        <a class="button-link secondary" href="/issues/new">"Open an issue"</a>
                    </div>
                </section>
            })?
        }
        Err(error) => {
            (view! {
                <div class="error-panel">
                    <strong>"Could not read forge data"</strong>
                    <p>(error)</p>
                </div>
            })?
        }
    };

    view! { shell(active: Tab::Dashboard, title: "Dashboard", keyword: None, child: content) }
}
