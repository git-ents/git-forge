//! Annotations route (Handoff v5): "the unified surface for every comment,
//! anchored or free-floating, repo-wide," filterable by binding health.
//!
//! The binding-health read reuses `pages::files::comment_confidence` --
//! the same real bound/rebound/orphaned classification the Code route
//! already shows per-file, computed here against every comment in the
//! repository rather than just the ones on one open file. There is no
//! oracle re-projection engine or attest-claim backend in gix-forge yet
//! (see `github.md`'s `pinned` state and `README.md`'s "repair mode"), so
//! this route stops at the same three real states Code uses -- bound,
//! rebound, orphaned -- plus "unattached" for comments with no position
//! binding at all. "Pin" and "mark obsolete" below are preview-only, like
//! `pages::releases`' claim signing and `pages::local`'s push: they run the
//! shared confirm step against the ref that a real attest claim would
//! occupy, but write nothing.

use gix_forge::{Binding, Comment, EntityOps};
use std::collections::HashMap;
use topcoat::{
    Result,
    context::Cx,
    router::{content::Form, page, path_param, query_params},
    view::view,
};

use crate::{
    auth::authorization,
    confirm::confirm_step,
    pages::{comments::subject_href, files::Confidence, with_repo},
    shell::{Tab, route_shell, shell},
};

#[path_param(error = bad_request)]
struct AnnotationId(String);

#[query_params(error = bad_request)]
struct AnnotationsQuery {
    state: Option<String>,
}

/// One row's worth of real data: the comment plus its binding-health read,
/// `None` only for comments with no position binding at all.
struct AnnotationRow {
    comment: Comment,
    confidence: Option<Confidence>,
}

impl AnnotationRow {
    fn state_key(&self) -> &'static str {
        match self.confidence {
            Some(Confidence::Bound) => "bound",
            Some(Confidence::Drifted) => "drifted",
            Some(Confidence::Orphaned) => "orphaned",
            Some(Confidence::Pinned) => "pinned",
            None => "unattached",
        }
    }

    fn state_label(&self) -> &'static str {
        match self.confidence {
            Some(confidence) => confidence.label(),
            None => "unattached",
        }
    }

    fn state_class(&self) -> &'static str {
        match self.confidence {
            Some(confidence) => confidence.css_class(),
            None => "confidence-unattached",
        }
    }

    fn anchor_path(&self) -> Option<&str> {
        match self.comment.binding.as_ref() {
            Some(Binding::Position(anchor)) => Some(anchor.identity.path.as_str()),
            _ => None,
        }
    }
}

/// Every comment in the repository, anchored or not, each with a real
/// binding-health read against the current `HEAD`. Best-effort per anchor:
/// a path that no longer resolves at `HEAD` (moved, deleted, or the whole
/// tree lookup failing) reads as `Orphaned` directly, matching what
/// `comment_confidence` itself would conclude if it could see the miss.
fn load_annotation_rows(
    repo: &gix::Repository,
) -> std::result::Result<Vec<AnnotationRow>, gix_forge::Error> {
    let head = repo.head_id().ok().map(|id| id.detach());
    let ids = Comment::list(repo)?;
    let rows = ids
        .into_iter()
        .filter_map(|id| Comment::load_from_repo(repo, &id).ok().flatten())
        .map(|comment| {
            let confidence = head
                .map(|head| Comment::annotation_state(repo, &comment, head))
                .transpose()?
                .flatten()
                .map(Confidence::from);
            Ok(AnnotationRow {
                comment,
                confidence,
            })
        })
        .collect::<std::result::Result<Vec<_>, gix_forge::Error>>()?;
    Ok(rows)
}

fn filter_rows<'rows>(
    rows: &'rows [AnnotationRow],
    state: Option<&str>,
) -> Vec<&'rows AnnotationRow> {
    match state {
        Some(state) if !state.is_empty() && state != "all" => {
            rows.iter().filter(|row| row.state_key() == state).collect()
        }
        _ => rows.iter().collect(),
    }
}

fn state_filter_href(state: &str) -> String {
    if state == "all" {
        "/annotations".to_owned()
    } else {
        format!("/annotations?state={state}")
    }
}

/// The nav pane: a filter strip over the real binding-health states, then
/// one row per matching comment -- subject (or "free-floating"), a snippet
/// of the anchored path when there is one, and the state badge.
async fn annotation_nav(
    cx: &Cx,
    rows: &[AnnotationRow],
    active_state: &str,
    selected: Option<&str>,
) -> Result {
    let counts: HashMap<&'static str, usize> =
        rows.iter().fold(HashMap::new(), |mut counts, row| {
            *counts.entry(row.state_key()).or_insert(0) += 1;
            counts
        });
    let filtered = filter_rows(rows, Some(active_state));
    view! { cx =>
        <div class="route-nav-header">
            <h2>"Annotations"</h2>
            <span class="route-nav-note">(rows.len()) " total"</span>
        </div>
        <div class="annotation-filters">
            for (key, label) in [("all", "All"), ("bound", "Bound"), ("drifted", "Drifted"), ("orphaned", "Orphaned"), ("pinned", "Pinned"), ("unattached", "Unattached")] {
                let count = if key == "all" { rows.len() } else { *counts.get(key).unwrap_or(&0) };
                let class = if active_state == key { "annotation-filter active" } else { "annotation-filter" };
                <a class=(class) href=(state_filter_href(key))>(label) " " <span class="muted">(count)</span></a>
            }
        </div>
        if filtered.is_empty() {
            <p class="route-nav-note">"No annotations match this filter."</p>
        } else {
            for row in filtered {
                let row_class = if selected == Some(row.comment.id.as_str()) { "nav-row active" } else { "nav-row" };
                let subject_label = row.comment.subject.as_deref().unwrap_or("free-floating");
                <a class=(row_class) href=(format!("/annotations/{}", row.comment.id))>
                    <div class="nav-row-top">
                        <span class="nav-row-title">(row.comment.author.as_str())</span>
                        <span class=(format!("confidence-label {}", row.state_class()))>(row.state_label())</span>
                    </div>
                    <span class="nav-row-sub">
                        (subject_label)
                        if let Some(path) = row.anchor_path() {
                            " · " <code class="oid">(path)</code>
                        }
                    </span>
                </a>
            }
        }
    }
}

async fn annotation_placeholder() -> Result {
    view! {
        <div class="placeholder-detail">
            <p class="eyebrow">"ANNOTATION DETAIL"</p>
            <h2>"Select an annotation"</h2>
            <p class="muted">"Choose a comment from the list to see its binding, location, and repair actions."</p>
        </div>
    }
}

/// The detail pane for one annotation: the comment body, its subject and
/// (if anchored) location, and -- for drifted/orphaned items -- the
/// preview-only repair actions.
async fn annotation_detail(cx: &Cx, row: &AnnotationRow) -> Result {
    let comment = &row.comment;
    let needs_repair = matches!(
        row.confidence,
        Some(Confidence::Drifted) | Some(Confidence::Orphaned)
    );
    let subject_label = comment.subject.as_deref();
    let subject_link = comment.subject.as_deref().and_then(subject_href);
    view! { cx =>
        <article class="route-main-width-md">
            <div class="detail-meta">
                <span class=(format!("status {}", row.state_class()))>(row.state_label())</span>
                <span class="muted">"#" (comment.id.as_str())</span>
            </div>
            <h1>"Comment by " (comment.author.as_str())</h1>
            if let Some(link) = &subject_link {
                <p class="muted">"Attached to " <a href=(link.as_str())>(subject_label.unwrap_or_default())</a></p>
            } else if let Some(label) = subject_label {
                <p class="muted">"Attached to " (label)</p>
            } else {
                <p class="muted">"Free-floating discussion"</p>
            }
            if let Some(Binding::Position(anchor)) = comment.binding.as_ref() {
                <dl class="facts">
                    <dt>"Anchor"</dt><dd><code class="oid">(anchor.id().map(|id| id.to_string()).unwrap_or_else(|_| "unavailable".to_owned()))</code></dd>
                    <dt>"Path"</dt><dd><code class="oid">(anchor.identity.path.as_str())</code></dd>
                    <dt>"Captured at"</dt><dd><code class="oid">(gix::ObjectId::from(anchor.identity.genesis_rev).to_string())</code></dd>
                    <dt>"Span"</dt><dd>(anchor.identity.span.start) "–" (anchor.identity.span.end)</dd>
                </dl>
            }
            <div class="body-copy">(comment.body.as_str())</div>
            <p class="action-note">"Edit or delete this comment from " <a href=(format!("/comments/{}", comment.id))>"its comment page"</a> "."</p>
            if needs_repair {
                <h3>"Repair mode"</h3>
                <p class="muted">"Repair resolves the real bind/5 oracle chain. Pin and obsolete actions publish signed attest claims after confirmation."</p>
                <div class="detail-actions">
                    <form class="inline-form" action=(format!("/annotations/pin/{}", comment.id)) method="post">
                        <button type="submit">"Sign rebind pin"</button>
                    </form>
                    <form class="inline-form" action=(format!("/annotations/obsolete/{}", comment.id)) method="post">
                        <button class="button-link secondary" type="submit">"Sign obsolete claim"</button>
                    </form>
                </div>
            }
        </article>
    }
}

async fn render(cx: &Cx, active_state: &str, selected: Option<&str>) -> Result {
    let data = with_repo(cx, load_annotation_rows).await;
    let (nav, detail) = match data {
        Ok(rows) => {
            let selected_row = selected.and_then(|id| rows.iter().find(|row| row.comment.id == id));
            let nav = annotation_nav(cx, &rows, active_state, selected).await?;
            let detail = match selected_row {
                Some(row) => annotation_detail(cx, row).await?,
                None => annotation_placeholder().await?,
            };
            (nav, detail)
        }
        Err(error) => (
            (view! { cx => <p class="error-text">(error)</p> })?,
            (view! { cx => <p class="error-text">"Could not load annotations."</p> })?,
        ),
    };
    view! { cx => route_shell(active: Tab::Annotations, title: "Annotations", nav_width: "22rem", nav: nav, detail: detail) }
}

#[page("/annotations")]
async fn annotations(cx: &Cx) -> Result {
    let query = query_params::<AnnotationsQuery>(cx)?;
    let state = query.state.as_deref().unwrap_or("all").to_owned();
    render(cx, &state, None).await
}

#[page("/annotations/{*annotation_id}")]
async fn annotation_detail_page(cx: &Cx) -> Result {
    let id = path_param::<AnnotationId>(cx)?.clone().to_string();
    render(cx, "all", Some(id.as_str())).await
}

#[page(POST "/annotations/pin/{*annotation_id}")]
async fn annotation_pin(cx: &Cx, Form(input): Form<HashMap<String, String>>) -> Result {
    let authorization = authorization(cx).await?;
    let id = path_param::<AnnotationId>(cx)?.clone();
    let summary = (view! { cx =>
        "Resolve the best bind/5 candidate for " <code>(id.as_str())</code> " and sign a rebind pin."
    })?;
    if let Some(confirm) = confirm_step(
        cx,
        &input,
        &format!("/annotations/pin/{id}"),
        &format!("refs/claims/annotation/{id}/rebind-pin"),
        &format!("/annotations/{id}"),
        summary,
    )
    .await?
    {
        return render_with_confirm(cx, "Confirm pin", confirm).await;
    }
    let write_id = id.clone();
    let result = with_repo(cx, move |repo| {
        let head = repo
            .head_id()
            .map_err(|error| gix_forge::Error::QueryRules(error.to_string()))?
            .detach();
        let comment = Comment::load_from_repo(repo, &write_id)?.ok_or_else(|| {
            gix_forge::Error::QueryRules(format!("annotation `{write_id}` was not found"))
        })?;
        Comment::rebind_pin_as(repo, &authorization, &comment, head).map(|_| ())
    })
    .await;
    match result {
        Ok(()) => render(cx, "all", Some(id.as_str())).await,
        Err(error) => {
            let content = (view! { cx => <div class="error-panel"><strong>"Repair failed"</strong><p>(error)</p></div> })?;
            view! { shell(active: Some(Tab::Annotations), title: "Repair failed", keyword: None, child: content) }
        }
    }
}

#[page(POST "/annotations/obsolete/{*annotation_id}")]
async fn annotation_obsolete(cx: &Cx, Form(input): Form<HashMap<String, String>>) -> Result {
    let authorization = authorization(cx).await?;
    let id = path_param::<AnnotationId>(cx)?.clone();
    let summary = (view! { cx =>
        "Sign an obsolete claim for annotation " <code>(id.as_str())</code> "."
    })?;
    if let Some(confirm) = confirm_step(
        cx,
        &input,
        &format!("/annotations/obsolete/{id}"),
        &format!("refs/claims/annotation/{id}/obsolete"),
        &format!("/annotations/{id}"),
        summary,
    )
    .await?
    {
        return render_with_confirm(cx, "Confirm obsolete", confirm).await;
    }
    let write_id = id.clone();
    let result = with_repo(cx, move |repo| {
        let comment = Comment::load_from_repo(repo, &write_id)?.ok_or_else(|| {
            gix_forge::Error::QueryRules(format!("annotation `{write_id}` was not found"))
        })?;
        Comment::obsolete_as(repo, &authorization, &comment).map(|_| ())
    })
    .await;
    match result {
        Ok(()) => render(cx, "all", Some(id.as_str())).await,
        Err(error) => {
            let content = (view! { cx => <div class="error-panel"><strong>"Repair failed"</strong><p>(error)</p></div> })?;
            view! { shell(active: Some(Tab::Annotations), title: "Repair failed", keyword: None, child: content) }
        }
    }
}

async fn render_with_confirm(cx: &Cx, title: &str, confirm: topcoat::view::View) -> Result {
    view! { cx => crate::shell::shell(active: Some(Tab::Annotations), title: title, keyword: None, child: confirm) }
}
