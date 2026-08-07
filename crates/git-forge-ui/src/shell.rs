use topcoat::{
    Result,
    asset::asset,
    view::{View, component, view},
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tab {
    Dashboard,
    Issues,
    Reviews,
    Members,
    Query,
}

impl Tab {
    const ALL: [Self; 5] = [
        Self::Dashboard,
        Self::Issues,
        Self::Reviews,
        Self::Members,
        Self::Query,
    ];

    const fn label(self) -> &'static str {
        match self {
            Self::Dashboard => "Dashboard",
            Self::Issues => "Issues",
            Self::Reviews => "Reviews",
            Self::Members => "Members",
            Self::Query => "Query",
        }
    }

    const fn icon(self) -> &'static str {
        match self {
            Self::Dashboard => "⌂",
            Self::Issues => "!",
            Self::Reviews => "✓",
            Self::Members => "●",
            Self::Query => "?",
        }
    }

    const fn href(self) -> &'static str {
        match self {
            Self::Dashboard => "/",
            Self::Issues => "/issues",
            Self::Reviews => "/reviews",
            Self::Members => "/members",
            Self::Query => "/query",
        }
    }
}

#[component]
pub(crate) async fn shell(active: Tab, title: &str, child: View) -> Result {
    view! {
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
                    <aside class="rail">
                        <a class="brand" href="/">"gf"</a>
                        <nav class="nav-list" aria-label="Primary">
                            for tab in Tab::ALL {
                                <a
                                    class=(if active == tab { "nav-link active" } else { "nav-link" })
                                    href=(tab.href())
                                >
                                    <span class="nav-icon">(tab.icon())</span>
                                    <span>(tab.label())</span>
                                </a>
                            }
                        </nav>
                    </aside>
                    <div class="workspace">
                        <header class="topbar">
                            <a class="repo-name" href="/">"git forge"</a>
                            <form class="search-form" action="/search" method="post">
                                <label class="sr-only" for="shell-search">"Search"</label>
                                <input id="shell-search" name="keyword" placeholder="Search issues and reviews" type="search">
                                <button type="submit">"Search"</button>
                            </form>
                        </header>
                        <main class="content">
                            <div class="page-heading">
                                <p class="eyebrow">"LOCAL REPOSITORY"</p>
                                <h1>(title)</h1>
                            </div>
                            (child)
                        </main>
                    </div>
                </div>
            </body>
        </html>
    }
}

#[component]
pub(crate) async fn split_shell(active: Tab, title: &str, list: View, detail: View) -> Result {
    view! {
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
                    <aside class="rail">
                        <a class="brand" href="/">"gf"</a>
                        <nav class="nav-list" aria-label="Primary">
                            for tab in Tab::ALL {
                                <a
                                    class=(if active == tab { "nav-link active" } else { "nav-link" })
                                    href=(tab.href())
                                >
                                    <span class="nav-icon">(tab.icon())</span>
                                    <span>(tab.label())</span>
                                </a>
                            }
                        </nav>
                    </aside>
                    <div class="workspace">
                        <header class="topbar">
                            <a class="repo-name" href="/">"git forge"</a>
                            <form class="search-form" action="/search" method="post">
                                <label class="sr-only" for="split-search">"Search"</label>
                                <input id="split-search" name="keyword" placeholder="Search issues and reviews" type="search">
                                <button type="submit">"Search"</button>
                            </form>
                        </header>
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
                </div>
            </body>
        </html>
    }
}
