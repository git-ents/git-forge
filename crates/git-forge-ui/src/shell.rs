use topcoat::{
    Result,
    asset::asset,
    context::Cx,
    cookie::{Cookies, cookies},
    view::{View, component, view},
};

use crate::{
    auth::authenticated_member,
    pages::{local_strip, palette_mount, palette_trigger},
};

/// The eight rail routes (Handoff v5). Query stays reachable only through
/// the command palette, so pages that render it (and other utility pages
/// like free-floating comment CRUD) pass `None` as `shell`'s `active` tab
/// rather than a `Tab` variant.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tab {
    Inbox,
    Code,
    Issues,
    Reviews,
    Annotations,
    Gates,
    Releases,
    Trust,
}

impl Tab {
    const ALL: [Self; 8] = [
        Self::Inbox,
        Self::Code,
        Self::Issues,
        Self::Reviews,
        Self::Annotations,
        Self::Gates,
        Self::Releases,
        Self::Trust,
    ];

    const fn label(self) -> &'static str {
        match self {
            Self::Inbox => "Inbox",
            Self::Code => "Code",
            Self::Issues => "Issues",
            Self::Reviews => "Reviews",
            Self::Annotations => "Annotations",
            Self::Gates => "Gates",
            Self::Releases => "Releases",
            Self::Trust => "Trust",
        }
    }

    /// Route paths keep their existing prefixes where a route already
    /// existed pre-handoff (Code stays under `/tree`, Trust under
    /// `/members`) so permalinks and nested routes (blob views, member
    /// edit forms, ...) don't need to move.
    const fn href(self) -> &'static str {
        match self {
            Self::Inbox => "/inbox",
            Self::Code => "/tree",
            Self::Issues => "/issues",
            Self::Reviews => "/reviews",
            Self::Annotations => "/annotations",
            Self::Gates => "/gates",
            Self::Releases => "/releases",
            Self::Trust => "/members",
        }
    }

    /// `<path d>` data for this tab's rail icon, lifted verbatim from the
    /// design mock's `sectionDefs` array. Inherited stroke via
    /// `currentColor`.
    const fn icon_path(self) -> &'static str {
        match self {
            Self::Inbox => {
                "M1.5 9.5h3.2l1 2h4.6l1-2h3.2M1.5 9.5 3 3h10l1.5 6.5v3.2a1 1 0 0 1-1 1h-11a1 1 0 0 1-1-1z"
            }
            Self::Code => "M5.8 4.2 2 8l3.8 3.8M10.2 4.2 14 8l-3.8 3.8",
            Self::Issues => "M8 1.6a6.4 6.4 0 1 0 0 12.8A6.4 6.4 0 0 0 8 1.6M8 4.8v4.1M8 11.3h.01",
            Self::Reviews => "M2 3h12v8.2H8.6L5.4 14v-2.8H2zM5 6.2h6M5 8.4h3.6",
            Self::Annotations => {
                "M8 5.6v8.2M8 5.6a1.6 1.6 0 1 0 0-3.2a1.6 1.6 0 0 0 0 3.2M2.8 9.2a5.2 5.2 0 0 0 10.4 0M1.8 9.2h2M12.2 9.2h2"
            }
            Self::Gates => {
                "M8 1.6 13.6 3.6v4.1c0 3.5-2.4 6-5.6 6.7C4.8 13.7 2.4 11.2 2.4 7.7V3.6zM5.9 7.9 7.4 9.4 10.3 6.3"
            }
            Self::Releases => {
                "M2 8.6 7.6 3h4.2L14 5.2v4.2L8.4 14 2 8.6zM9.7 6.3a1 1 0 1 0 0-2a1 1 0 0 0 0 2"
            }
            Self::Trust => {
                "M8 1.6 13.4 3.7v4c0 3.6-2.3 6.1-5.4 6.7-3.1-.6-5.4-3.1-5.4-6.7v-4zM5.7 8l1.6 1.6L10.3 6"
            }
        }
    }

    /// Rail count badge. Issues/Reviews query the same `*_status` goals
    /// `dashboard.rs` used to; the other tabs have no cheap real count yet.
    async fn badge(self, cx: &Cx) -> usize {
        match self {
            Self::Issues => open_count(cx, "issue_status(Id, Status)").await,
            Self::Reviews => open_count(cx, "review_status(Id, Status)").await,
            Self::Inbox
            | Self::Code
            | Self::Annotations
            | Self::Gates
            | Self::Releases
            | Self::Trust => 0,
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

/// Commits the current branch is ahead of / behind its upstream, or `None`
/// when `HEAD` has no configured upstream (e.g. a detached checkout, or a
/// branch that was never pushed/tracked) — in which case the caller omits
/// the indicator entirely rather than showing a stand-in value.
async fn ahead_behind(cx: &Cx) -> Option<(usize, usize)> {
    crate::pages::with_repo(cx, |repo| {
        Ok((|| {
            let head = repo.head_ref().ok().flatten()?;
            let upstream_name = head
                .remote_tracking_ref_name(gix::remote::Direction::Fetch)?
                .ok()?
                .into_owned();
            let upstream_id = repo.find_reference(&upstream_name).ok()?.into_fully_peeled_id().ok()?;
            let head_id = head.into_fully_peeled_id().ok()?;
            if head_id == upstream_id {
                return Some((0, 0));
            }
            let ahead = repo
                .rev_walk([head_id])
                .with_hidden([upstream_id])
                .all()
                .ok()?
                .count();
            let behind = repo
                .rev_walk([upstream_id])
                .with_hidden([head_id])
                .all()
                .ok()?
                .count();
            Some((ahead, behind))
        })())
    })
    .await
    .unwrap_or(None)
}

/// The three-state theme cycle (non-negotiable #9): default follows
/// `prefers-color-scheme` ("auto", no `data-theme` attribute rendered),
/// and can be pinned explicitly. This app ships no client JavaScript, so
/// the toggle is a plain POST form (see [`theme_set`]) that writes a
/// `theme` cookie and redirects back -- not a `localStorage` write.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ThemeMode {
    Auto,
    Light,
    Dark,
}

impl ThemeMode {
    fn from_cookie(cx: &Cx) -> Self {
        match cookies(cx).get("theme").map(|c| c.value().to_owned()).as_deref() {
            Some("light") => Self::Light,
            Some("dark") => Self::Dark,
            _ => Self::Auto,
        }
    }

    const fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }

    const fn next(self) -> Self {
        match self {
            Self::Auto => Self::Light,
            Self::Light => Self::Dark,
            Self::Dark => Self::Auto,
        }
    }

    const fn title(self) -> &'static str {
        match self {
            Self::Auto => "Theme: auto (follows system) -- click for light",
            Self::Light => "Theme: light -- click for dark",
            Self::Dark => "Theme: dark -- click for auto",
        }
    }

    /// Lifted from the design mock's `themeIcons`.
    const fn icon_path(self) -> &'static str {
        match self {
            Self::Auto => "M8 1.6a6.4 6.4 0 1 0 0 12.8A6.4 6.4 0 0 0 8 1.6M8 1.6v12.8A6.4 6.4 0 0 0 8 1.6z",
            Self::Light => {
                "M8 4.6a3.4 3.4 0 1 0 0 6.8a3.4 3.4 0 0 0 0-6.8M8 1v1.4M8 13.6V15M15 8h-1.4M2.4 8H1M12.9 3.1l-1 1M4.1 11.9l-1 1M12.9 12.9l-1-1M4.1 4.1l-1-1"
            }
            Self::Dark => "M13.4 9.6A5.8 5.8 0 0 1 6.4 2.6a5.9 5.9 0 1 0 7 7z",
        }
    }
}

/// The `data-theme` attribute value to stamp on `<html>`, or `None` for
/// "auto" (left to the `prefers-color-scheme` media query in `style.css`).
fn theme_attr(mode: ThemeMode) -> Option<&'static str> {
    match mode {
        ThemeMode::Auto => None,
        ThemeMode::Light => Some("light"),
        ThemeMode::Dark => Some("dark"),
    }
}

#[component]
async fn theme_toggle(cx: &Cx, mode: ThemeMode, from: &str) -> Result {
    let next = mode.next();
    view! { cx =>
        <form class="inline-form theme-toggle-form" action="/theme" method="post">
            <input type="hidden" name="mode" value=(next.as_str())>
            <input type="hidden" name="from" value=(from)>
            <button class="theme-toggle" type="submit" title=(mode.title())>
                <svg width="14" height="14" viewBox="0 0 16 16" aria-hidden="true"><path d=(mode.icon_path())></path></svg>
            </button>
        </form>
    }
}

#[component]
async fn rail(cx: &Cx, active: Option<Tab>) -> Result {
    view! { cx =>
        <aside class="rail">
            <a class="brand" href="/inbox" title="git forge">"gf"</a>
            <nav class="nav-list" aria-label="Primary">
                for tab in Tab::ALL {
                    let is_active = active == Some(tab);
                    let badge = tab.badge(cx).await;
                    <a
                        class=(if is_active { "nav-link active" } else { "nav-link" })
                        href=(tab.href())
                        title=(tab.label())
                    >
                        <svg width="16" height="16" viewBox="0 0 16 16" aria-hidden="true"><path d=(tab.icon_path())></path></svg>
                        <span class="sr-only">(tab.label())</span>
                        if badge > 0 {
                            <span class="nav-badge">(badge)</span>
                        }
                    </a>
                }
            </nav>
            <span class="rail-spacer"></span>
            <a class="rail-icon-btn" href="#palette" title="Command palette" aria-haspopup="dialog" aria-controls="palette">
                <svg width="16" height="16" viewBox="0 0 16 16" aria-hidden="true"><path d="M4 6l2.5 2L4 10M8.5 10.5h3.5"></path><rect x="1.5" y="2.5" width="13" height="11" rx="1.5"></rect></svg>
            </a>
        </aside>
    }
}

#[component]
async fn topbar(cx: &Cx, keyword: Option<&str>, from: &str) -> Result {
    let member = authenticated_member(cx).await;
    let branch = repo_branch(cx).await;
    let ahead_behind = ahead_behind(cx).await;
    let mode = ThemeMode::from_cookie(cx);
    view! { cx =>
        <header class="topbar">
            <a class="repo-name" href="/inbox">"git forge"</a>
            <span class="branch-pill">(branch)</span>
            if let Some((ahead, behind)) = ahead_behind {
                <span class="ahead-behind" title="Ahead/behind against upstream">
                    (format!("↑{ahead} ↓{behind}"))
                </span>
            }
            <form class="search-form" action="/search" method="post">
                <label class="sr-only" for="shell-search">"Search"</label>
                <input id="shell-search" name="keyword" value=(keyword.unwrap_or_default()) placeholder="Search, or ?predicate" type="search">
                <button type="submit">"Search"</button>
            </form>
            (palette_trigger(cx).await?)
            theme_toggle(mode: mode, from: from)
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

/// Utility-page shell (search, query, palette, member/comment forms, ...):
/// a single scrolling pane with a plain page title. None of these are part
/// of the eight Handoff-v5 routes, so they keep the pre-handoff heading
/// convention rather than each hand-rolling their own `<h1>`.
#[component]
pub(crate) async fn shell(
    cx: &Cx,
    active: Option<Tab>,
    title: &str,
    keyword: Option<&str>,
    child: View,
) -> Result {
    let mode = ThemeMode::from_cookie(cx);
    view! { cx =>
        <!DOCTYPE html>
        <html lang="en" data-theme=(theme_attr(mode).unwrap_or_default())>
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
                        topbar(keyword: keyword, from: active.map(Tab::href).unwrap_or("/inbox"))
                        <main class="content content-plain">
                            <h1 class="page-title">(title)</h1>
                            (child)
                        </main>
                        (local_strip(cx).await?)
                    </div>
                    (palette_mount(cx).await?)
                </div>
            </body>
        </html>
    }
}

/// Full-width single-pane shell for a route that owns its whole heading
/// (currently just Inbox: title, item count, and "last opened" all live in
/// one line per the design mock, which a generic `<h1>{title}</h1>` can't
/// express).
#[component]
pub(crate) async fn full_shell(cx: &Cx, active: Tab, title: &str, child: View) -> Result {
    let mode = ThemeMode::from_cookie(cx);
    view! { cx =>
        <!DOCTYPE html>
        <html lang="en" data-theme=(theme_attr(mode).unwrap_or_default())>
            <head>
                <meta charset="utf-8">
                <meta name="viewport" content="width=device-width, initial-scale=1">
                <title>"git forge · " (title)</title>
                <link rel="stylesheet" href=(asset!("../assets/style.css"))>
            </head>
            <body>
                <div class="app-shell">
                    rail(active: Some(active))
                    <div class="workspace">
                        topbar(keyword: None, from: active.href())
                        <main class="content content-plain">
                            (child)
                        </main>
                        (local_strip(cx).await?)
                    </div>
                    (palette_mount(cx).await?)
                </div>
            </body>
        </html>
    }
}

/// The master/detail shell used by every route with a nav list + detail
/// pane (Code, Issues, Reviews, Annotations, Gates, Releases, Trust). Each
/// route owns its own sticky nav header (title + per-route affordance)
/// rather than a shared page-heading block, matching the design mock.
#[component]
pub(crate) async fn route_shell(
    cx: &Cx,
    active: Tab,
    title: &str,
    nav_width: &str,
    nav: View,
    detail: View,
) -> Result {
    let mode = ThemeMode::from_cookie(cx);
    view! { cx =>
        <!DOCTYPE html>
        <html lang="en" data-theme=(theme_attr(mode).unwrap_or_default())>
            <head>
                <meta charset="utf-8">
                <meta name="viewport" content="width=device-width, initial-scale=1">
                <title>"git forge · " (title)</title>
                <link rel="stylesheet" href=(asset!("../assets/style.css"))>
            </head>
            <body>
                <div class="app-shell">
                    rail(active: Some(active))
                    <div class="workspace">
                        topbar(keyword: None, from: active.href())
                        <div class="route-layout" style=(format!("grid-template-columns:{nav_width} minmax(0,1fr)"))>
                            <nav class="route-nav">(nav)</nav>
                            <main class="route-main">(detail)</main>
                        </div>
                        (local_strip(cx).await?)
                    </div>
                    (palette_mount(cx).await?)
                </div>
            </body>
        </html>
    }
}
