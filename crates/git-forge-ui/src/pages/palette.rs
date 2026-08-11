//! The command palette (Packet F): a cross-entity "jump to" search.
//!
//! This app ships no client JavaScript, so `⌘K` cannot be a real key
//! binding. The palette opens instead through a plain, focusable trigger
//! link (`href="#palette"`) and the CSS `:target` pattern already used
//! elsewhere in this codebase for modal-like UI: `#palette` is hidden by
//! default and shown when it is the URL's fragment. Closing re-targets the
//! same link back to `#`.
//!
//! Because there is no client-side runtime, "live" filtering is a real GET
//! round trip to [`palette_page`]. A URL fragment is never sent to the
//! server, so a plain form submission to `/palette` would drop `#palette`
//! and the overlay would render populated but invisible (still hidden by
//! `:target`). To keep the just-searched overlay visible, [`palette_mount`]
//! instead forces it open with a plain `is-open` class whenever a `jump`
//! query parameter is present, regardless of the fragment -- see
//! `DESIGN-SPEC.md`/`HANDOFF.md` packet F, option (a).
//!
//! Entities indexed: branch refs (via `gix::Repository::references`), repo
//! paths (only when the query looks path-like, and only a bounded walk of
//! the `HEAD` tree -- see [`PATH_WALK_BUDGET`]/[`PATH_MATCH_CAP`]), issues
//! (title/id substring), and reviews (id/body substring). This is a
//! single-repository forge, so there is no repo picker entry.

use gix::bstr::ByteSlice;
use gix_forge::{EntityOps, Issue, Review};
use topcoat::{
    Result,
    context::Cx,
    router::{page, query_params},
    view::view,
};

use crate::{
    pages::with_repo,
    shell::{Tab, shell},
};

/// Total results shown across every entity type, matching the design
/// mock's `paletteResults.slice(0, 8)`.
const RESULT_CAP: usize = 8;

/// A path-like query walks `HEAD`'s tree looking for matches; stop after
/// this many matching paths even if the tree has more.
const PATH_MATCH_CAP: usize = 50;

/// Independent of matches found, stop walking the tree after touching this
/// many entries. A full-repo tree walk is not a reasonable per-request cost
/// for a type-ahead-shaped feature; this bounds the worst case (a huge repo
/// with a path-like query that matches nothing) to a fixed amount of work.
const PATH_WALK_BUDGET: usize = 20_000;

#[query_params(error = bad_request)]
struct PaletteQuery {
    jump: Option<String>,
}

struct Hit {
    kind: &'static str,
    label: String,
    sub: String,
    href: String,
}

/// The topbar's click target for opening the palette. A visible button
/// standing in for the `⌘K`/`Ctrl-K` shortcut the design mock assumes a JS
/// runtime can bind -- this app has none, so "press a key" becomes "click a
/// link".
pub(crate) async fn palette_trigger(_cx: &Cx) -> Result {
    view! {
        <a class="palette-trigger" href="#palette" aria-haspopup="dialog" aria-controls="palette">
            <span>"Jump"</span>
            <kbd>"⌘K"</kbd>
        </a>
    }
}

/// The overlay itself, mounted once per page by `shell`/`split_shell` so
/// the trigger link has something to target. Hidden by default; opened
/// either by `:target` (the fragment-driven click flow, no results yet) or
/// by a real `?jump=` search landing on `/palette` (forced open, see the
/// module docs).
pub(crate) async fn palette_mount(cx: &Cx) -> Result {
    let query = query_params::<PaletteQuery>(cx)?;
    let jump = query
        .jump
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let forced_open = jump.is_some();
    let close_href = if forced_open { "/" } else { "#" };
    let hits = match jump {
        Some(value) => search(cx, value).await,
        None => Vec::new(),
    };

    view! { cx =>
        <div id="palette" class=(if forced_open { "palette-overlay is-open" } else { "palette-overlay" })>
            <a class="palette-scrim" href=(close_href) aria-hidden="true" tabindex="-1"></a>
            <div class="palette-panel" role="dialog" aria-modal="true" aria-label="Jump to...">
                <form class="palette-form" action="/palette" method="get">
                    <input class="palette-input" type="search" name="jump" value=(jump.unwrap_or_default()) placeholder="jump: ref · path · issue · review" autocomplete="off" autofocus="">
                    <a class="palette-close" href=(close_href)>"Close"</a>
                </form>
                <div class="palette-results">
                    if let Some(value) = jump {
                        if hits.is_empty() {
                            <p class="palette-empty">"No matches for \"" (value) "\"."</p>
                        } else {
                            <ul class="palette-list">
                                for hit in &hits {
                                    <li>
                                        <a class="palette-row" href=(hit.href.as_str())>
                                            <span class="palette-chip">(hit.kind)</span>
                                            <span class="palette-label">(hit.label.as_str())</span>
                                            <span class="palette-sub">(hit.sub.as_str())</span>
                                        </a>
                                    </li>
                                }
                            </ul>
                        }
                    } else {
                        <p class="palette-hint">"Type a branch name, a file path, or an issue/review title or id."</p>
                    }
                </div>
            </div>
        </div>
    }
}

/// The dedicated `/palette` route: a real, bookmarkable page that renders
/// the same overlay forced open (via the `jump` query parameter), for
/// browsers or agents that land on the URL directly rather than clicking
/// the trigger. Escape has no key-binding equivalent here either; the
/// panel's "Close" link is the click-driven substitute.
#[page("/palette")]
async fn palette_page(cx: &Cx) -> Result {
    let content = (view! { cx =>
        <p class="muted">"Use the panel above to jump to a branch, a file path, or an issue or review."</p>
    })?;
    view! { shell(active: Tab::Query, title: "Jump", keyword: None, child: content) }
}

async fn search(cx: &Cx, query: &str) -> Vec<Hit> {
    let query = query.to_owned();
    with_repo(cx, move |repo| Ok(collect_hits(repo, &query)))
        .await
        .unwrap_or_default()
}

fn collect_hits(repo: &gix::Repository, query: &str) -> Vec<Hit> {
    let needle = query.to_ascii_lowercase();
    let mut hits = Vec::new();

    collect_ref_hits(repo, &needle, &mut hits);
    if hits.len() < RESULT_CAP && looks_path_like(query) {
        collect_path_hits(repo, &needle, &mut hits);
    }
    if hits.len() < RESULT_CAP {
        collect_issue_hits(repo, &needle, &mut hits);
    }
    if hits.len() < RESULT_CAP {
        collect_review_hits(repo, &needle, &mut hits);
    }

    hits
}

fn looks_path_like(query: &str) -> bool {
    query.contains('/') || query.contains('.')
}

fn collect_ref_hits(repo: &gix::Repository, needle: &str, hits: &mut Vec<Hit>) {
    let Ok(references) = repo.references() else {
        return;
    };
    let Ok(branches) = references.prefixed("refs/heads") else {
        return;
    };
    for reference in branches.flatten() {
        if hits.len() >= RESULT_CAP {
            return;
        }
        let full = lossy(reference.name().as_bstr().as_bytes());
        let short = full.strip_prefix("refs/heads/").unwrap_or(&full);
        if short.to_ascii_lowercase().contains(needle) {
            hits.push(Hit {
                kind: "ref",
                label: short.to_owned(),
                sub: "branch".to_owned(),
                href: format!("/tree?ref={}", percent_encode(full.as_bytes())),
            });
        }
    }
}

fn collect_path_hits(repo: &gix::Repository, needle: &str, hits: &mut Vec<Hit>) {
    let Ok(tree) = repo.head_tree() else {
        return;
    };
    let mut matches = Vec::new();
    let mut budget = PATH_WALK_BUDGET;
    walk_tree(repo, &tree, "", needle, &mut matches, &mut budget);
    for path in matches {
        if hits.len() >= RESULT_CAP {
            return;
        }
        hits.push(Hit {
            kind: "path",
            href: format!("/tree?path={}", percent_encode(path.as_bytes())),
            label: path,
            sub: "HEAD".to_owned(),
        });
    }
}

/// Depth-first walk of `tree`, collecting up to [`PATH_MATCH_CAP`] paths
/// containing `needle` while spending at most `budget` entry visits total
/// (decremented in place), whichever limit is hit first.
fn walk_tree(
    repo: &gix::Repository,
    tree: &gix::Tree<'_>,
    prefix: &str,
    needle: &str,
    matches: &mut Vec<String>,
    budget: &mut usize,
) {
    for entry in tree.iter() {
        if matches.len() >= PATH_MATCH_CAP || *budget == 0 {
            return;
        }
        *budget -= 1;
        let Ok(entry) = entry else { continue };
        let name = lossy(entry.filename().as_bytes());
        let path = if prefix.is_empty() {
            name
        } else {
            format!("{prefix}/{name}")
        };
        if path.to_ascii_lowercase().contains(needle) {
            matches.push(path.clone());
        }
        if entry.mode().kind() == gix::objs::tree::EntryKind::Tree
            && let Ok(subtree) = repo.find_tree(entry.object_id())
        {
            walk_tree(repo, &subtree, &path, needle, matches, budget);
        }
    }
}

fn collect_issue_hits(repo: &gix::Repository, needle: &str, hits: &mut Vec<Hit>) {
    let Ok(ids) = Issue::list(repo) else {
        return;
    };
    for id in ids {
        if hits.len() >= RESULT_CAP {
            return;
        }
        let Ok(Some(issue)) = Issue::load_from_repo(repo, &id) else {
            continue;
        };
        if issue.title.to_ascii_lowercase().contains(needle)
            || issue.id.to_ascii_lowercase().contains(needle)
        {
            hits.push(Hit {
                kind: "issue",
                label: issue.title,
                sub: format!("#{}", issue.id),
                href: format!("/issues/{}", issue.id),
            });
        }
    }
}

fn collect_review_hits(repo: &gix::Repository, needle: &str, hits: &mut Vec<Hit>) {
    let Ok(ids) = Review::list(repo) else {
        return;
    };
    for id in ids {
        if hits.len() >= RESULT_CAP {
            return;
        }
        let Ok(Some(review)) = Review::load_from_repo(repo, &id) else {
            continue;
        };
        if review.id.to_ascii_lowercase().contains(needle)
            || review.body.to_ascii_lowercase().contains(needle)
        {
            hits.push(Hit {
                kind: "review",
                label: format!("review {}", review.id),
                sub: review.target.to_string(),
                href: format!("/reviews/{}", review.id),
            });
        }
    }
}

fn lossy(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(text) => text.to_owned(),
        Err(_) => String::from_utf8_lossy(bytes).into_owned(),
    }
}

fn percent_encode(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(bytes.len());
    for &byte in bytes {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(byte as char);
        } else {
            encoded.push('%');
            encoded.push(
                char::from_digit((byte >> 4) as u32, 16)
                    .unwrap()
                    .to_ascii_uppercase(),
            );
            encoded.push(
                char::from_digit((byte & 0x0f) as u32, 16)
                    .unwrap()
                    .to_ascii_uppercase(),
            );
        }
    }
    encoded
}
