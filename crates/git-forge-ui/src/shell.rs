use topcoat::{
    Result,
    asset::asset,
    context::Cx,
    view::{View, component, view},
};

use crate::{
    auth::authenticated_member,
    pages::{palette_mount, palette_trigger},
};

/// The six rail routes. Query and Search stay reachable through the top-bar
/// search form instead of a rail entry, so pages that render them (and other
/// utility pages like free-floating comments) pass `None` as `shell`'s
/// `active` tab rather than a `Tab` variant.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tab {
    Files,
    Issues,
    Reviews,
    Releases,
    WorkingCopy,
    Config,
}

impl Tab {
    const ALL: [Self; 6] = [
        Self::Files,
        Self::Issues,
        Self::Reviews,
        Self::Releases,
        Self::WorkingCopy,
        Self::Config,
    ];

    const fn label(self) -> &'static str {
        match self {
            Self::Files => "Files",
            Self::Issues => "Issues",
            Self::Reviews => "Reviews",
            Self::Releases => "Releases",
            Self::WorkingCopy => "Working copy",
            Self::Config => "Config",
        }
    }

    const fn href(self) -> &'static str {
        match self {
            Self::Files => "/tree",
            Self::Issues => "/issues",
            Self::Reviews => "/reviews",
            Self::Releases => "/releases",
            Self::WorkingCopy => "/workingcopy",
            Self::Config => "/members",
        }
    }

    /// `<path d>` data for this tab's rail icon, inherited stroke via
    /// `currentColor`. Files/Issues/Reviews/Releases/WorkingCopy are lifted
    /// verbatim from the design mock's `sections` array; Config has no mock
    /// equivalent (the mock drops Members/Config from the rail) so it draws a
    /// plain gear glyph in the same stroke style.
    const fn icon_path(self) -> &'static str {
        match self {
            Self::Files => "M1.5 3.5h4l1.5 2h7.5v7h-13z",
            Self::Issues => {
                "M14 8a6 6 0 1 1-12 0a6 6 0 1 1 12 0M9.2 8a1.2 1.2 0 1 1-2.4 0a1.2 1.2 0 1 1 2.4 0"
            }
            Self::Reviews => {
                "M10.6 8a2.6 2.6 0 1 1-5.2 0a2.6 2.6 0 1 1 5.2 0M.5 8h4.9M10.6 8h4.9"
            }
            Self::Releases => {
                "M2 2h5.5L14 8.5 8.5 14 2 7.5zM6.7 5.5a1.2 1.2 0 1 1-2.4 0a1.2 1.2 0 1 1 2.4 0"
            }
            Self::WorkingCopy => "M8 13.5V5M4.5 8.5 8 5l3.5 3.5M2.5 13.5h11",
            Self::Config => {
                "M8 5.3a2.7 2.7 0 1 0 0 5.4 2.7 2.7 0 0 0 0-5.4M8 1v1.7M8 13.3V15M15 8h-1.7M2.7 8H1M12.6 3.4l-1.2 1.2M4.6 11.4l-1.2 1.2M12.6 12.6l-1.2-1.2M4.6 4.6 3.4 3.4"
            }
        }
    }

    /// Rail count badge. Issues/Reviews query the same `*_status` goals
    /// `dashboard.rs` used to; the other tabs have no cheap real count yet.
    async fn badge(self, cx: &Cx) -> usize {
        match self {
            Self::Issues => open_count(cx, "issue_status(Id, Status)").await,
            Self::Reviews => open_count(cx, "review_status(Id, Status)").await,
            Self::Files | Self::Releases | Self::WorkingCopy | Self::Config => 0,
        }
    }
}

async fn open_count(cx: &Cx, goal: &str) -> usize {
    let goal = goal.to_owned();
    crate::pages::with_repo(cx, move |repo| {
        let rows = gix_forge::query_goal(repo, &goal, &["Id", "Status"])?;
        Ok(rows
            .into_iter()
            .filter(|row| row[1].to_string() == "open")
            .count())
    })
    .await
    .unwrap_or(0)
}

/// The branch the repository's `HEAD` currently points at, or `"detached"`
/// when it doesn't name a branch.
async fn repo_branch(cx: &Cx) -> String {
    crate::pages::with_repo(cx, |repo| {
        Ok(repo
            .head_name()
            .ok()
            .flatten()
            .map(|name| name.shorten().to_string())
            .unwrap_or_else(|| "detached".to_owned()))
    })
    .await
    .unwrap_or_else(|_| "unknown".to_owned())
}

#[component]
async fn rail(cx: &Cx, active: Option<Tab>) -> Result {
    view! { cx =>
        <aside class="rail">
            <a class="brand" href="/tree" title="git forge">"gf"</a>
            <nav class="nav-list" aria-label="Primary">
                for tab in Tab::ALL {
                    let is_active = active == Some(tab);
                    let badge = tab.badge(cx).await;
                    <a
                        class=(if is_active { "nav-link active" } else { "nav-link" })
                        href=(tab.href())
                        title=(tab.label())
                    >
                        <svg width="17" height="17" viewBox="0 0 16 16" aria-hidden="true"><path d=(tab.icon_path())></path></svg>
                        <span class="sr-only">(tab.label())</span>
                        if badge > 0 {
                            <span class="nav-badge">(badge)</span>
                        }
                    </a>
                }
            </nav>
        </aside>
    }
}

#[component]
async fn topbar(cx: &Cx, keyword: Option<&str>) -> Result {
    let member = authenticated_member(cx).await;
    let branch = repo_branch(cx).await;
    view! { cx =>
        <header class="topbar">
            <a class="repo-name" href="/tree">"git forge"</a>
            <span class="branch-pill">(branch)</span>
            <span class="ahead-behind" title="Ahead/behind against upstream">"↑– ↓–"</span>
            <form class="search-form" action="/search" method="post">
                <label class="sr-only" for="shell-search">"Search"</label>
                <input id="shell-search" name="keyword" value=(keyword.unwrap_or_default()) placeholder="Search, or ?predicate" type="search">
                <button type="submit">"Search"</button>
            </form>
            (palette_trigger(cx).await?)
            <div class="auth-state">
                if let Some(member) = &member {
                    <span class="status-dot ok"></span>
                    <strong>(member.id.as_str())</strong>
                    <small>(member.role.as_str()) " · attributed to you"</small>
                } else {
                    <span class="status-dot"></span>
                    <strong>"Read-only"</strong>
                    <small>"Authenticate to make changes"</small>
                }
            </div>
        </header>
    }
}

#[component]
pub(crate) async fn shell(
    cx: &Cx,
    active: Option<Tab>,
    title: &str,
    keyword: Option<&str>,
    child: View,
) -> Result {
    view! { cx =>
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8">
                <meta name="viewport" content="width=device-width, initial-scale=1">
                <title>"git forge · " (title)</title>
                <link rel="stylesheet" href=(asset!("../assets/style.css"))>
            </head>
            <body>
                <div class="app-shell">
                    rail(active: active)
                    <div class="workspace">
                        topbar(keyword: keyword)
                        <main class="content">
                            <div class="page-heading">
                                <p class="eyebrow">"LOCAL REPOSITORY"</p>
                                <h1>(title)</h1>
                            </div>
                            (child)
                        </main>
                    </div>
                    (palette_mount(cx).await?)
                </div>
            </body>
        </html>
    }
}

#[component]
pub(crate) async fn split_shell(
    cx: &Cx,
    active: Option<Tab>,
    title: &str,
    list: View,
    detail: View,
) -> Result {
    view! { cx =>
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8">
                <meta name="viewport" content="width=device-width, initial-scale=1">
                <title>"git forge · " (title)</title>
                <link rel="stylesheet" href=(asset!("../assets/style.css"))>
            </head>
            <body>
                <div class="app-shell">
                    rail(active: active)
                    <div class="workspace">
                        topbar(keyword: None)
                        <main class="content">
                            <div class="page-heading">
                                <p class="eyebrow">"LOCAL REPOSITORY"</p>
                                <h1>(title)</h1>
                            </div>
                            <div class="split-pane">
                                <section class="master-pane">(list)</section>
                                <section class="detail-pane">(detail)</section>
                            </div>
                        </main>
                    </div>
                    (palette_mount(cx).await?)
                </div>
            </body>
        </html>
    }
}
