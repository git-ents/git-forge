use std::collections::HashMap;

use gix_forge::{Comment, EntityOps, Issue, Member, Review};
use topcoat::{
    Result,
    context::Cx,
    router::{content::Form, page, path_param},
    view::view,
};

use crate::{
    auth::{authorization, authorization_error, can_manage_members},
    confirm::confirm_step,
    pages::with_repo,
    shell::{Tab, route_shell, shell},
};

#[path_param(error = bad_request)]
struct MemberId(String);

#[derive(Clone)]
struct MemberForm {
    signing_key: String,
    role: String,
}

impl MemberForm {
    fn from_member(member: &Member) -> Self {
        Self {
            signing_key: member.signing_key.clone(),
            role: member.role.clone(),
        }
    }

    fn from_input(input: &HashMap<String, String>) -> Self {
        Self {
            signing_key: input.get("signing_key").cloned().unwrap_or_default(),
            role: input.get("role").cloned().unwrap_or_default(),
        }
    }
}

async fn member_form(cx: &Cx, action: &str, form: &MemberForm, error: Option<&str>) -> Result {
    view! { cx =>
        <section class="form-layout">
            <div class="form-intro">
                <p class="eyebrow">"REPOSITORY ACCESS"</p>
                <h2>"Add a trusted member"</h2>
                <p class="muted">"Members identify people who can participate in this forge."</p>
            </div>
            if let Some(error) = error {
                <div class="error-panel"><strong>"Member could not be saved"</strong><p>(error)</p></div>
            }
            <form class="entity-form" action=(action) method="post">
                <fieldset>
                    <legend>"Identity"</legend>
                    <label for="member-signing-key">"Signing key"</label>
                    <input id="member-signing-key" name="signing_key" value=(form.signing_key.as_str()) placeholder="OpenPGP or SSH-style key identity" required="">
                    <label for="member-role">"Role"</label>
                    <select id="member-role" name="role">
                        if form.role == "maintainer" {
                            <option value="member">"Member"</option>
                            <option value="reviewer">"Reviewer"</option>
                            <option value="maintainer" selected="">"Maintainer"</option>
                        } else if form.role == "reviewer" {
                            <option value="member">"Member"</option>
                            <option value="reviewer" selected="">"Reviewer"</option>
                            <option value="maintainer">"Maintainer"</option>
                        } else {
                            <option value="member" selected="">"Member"</option>
                            <option value="reviewer">"Reviewer"</option>
                            <option value="maintainer">"Maintainer"</option>
                        }
                    </select>
                    <p class="form-help">"The role is descriptive project metadata and can be changed later."</p>
                </fieldset>
                <div class="form-actions">
                    <a class="button-link secondary" href="/members">"Cancel"</a>
                    <button type="submit">"Save member"</button>
                </div>
            </form>
        </section>
    }
}

/// Load the full member directory the same eager way every other route in
/// this app loads its entity lists (no pagination or indexing anywhere
/// here): list ids, then load each one.
fn load_members(repo: &gix::Repository) -> std::result::Result<Vec<Member>, gix_forge::Error> {
    let ids = Member::list(repo)?;
    ids.into_iter()
        .map(|id| Member::load_from_repo(repo, &id))
        .collect::<std::result::Result<Vec<_>, _>>()
        .map(|members| members.into_iter().flatten().collect())
}

/// How many issues, reviews, and comments carry `member_id` as an author --
/// real, computed by loading every `Issue`/`Review`/`Comment` and filtering
/// in Rust, matching `palette.rs`'s `collect_issue_hits`/`collect_review_hits`
/// pattern. `Issue`/`Review` attribute authorship as a `Vec<String>`
/// (`reporters`/`requesters`); `Comment` uses a singular `author: String`.
struct AttributionCounts {
    issues: usize,
    reviews: usize,
    comments: usize,
}

fn count_attributions(
    repo: &gix::Repository,
    member_id: &str,
) -> std::result::Result<AttributionCounts, gix_forge::Error> {
    let issues = Issue::list(repo)?
        .into_iter()
        .filter_map(|id| Issue::load_from_repo(repo, &id).ok().flatten())
        .filter(|issue| issue.reporters.iter().any(|reporter| reporter == member_id))
        .count();
    let reviews = Review::list(repo)?
        .into_iter()
        .filter_map(|id| Review::load_from_repo(repo, &id).ok().flatten())
        .filter(|review| review.requesters.iter().any(|requester| requester == member_id))
        .count();
    let comments = Comment::list(repo)?
        .into_iter()
        .filter_map(|id| Comment::load_from_repo(repo, &id).ok().flatten())
        .filter(|comment| comment.author == member_id)
        .count();
    Ok(AttributionCounts {
        issues,
        reviews,
        comments,
    })
}

/// A short, honest description of what a role actually grants, per
/// `auth.rs` (the only role checked anywhere is `"maintainer"`; anything
/// else falls back to the non-maintainer capability set).
fn role_explain(role: &str) -> &'static str {
    match role {
        "maintainer" => {
            "Maintainers administer the repository: they can create, edit, and remove any entity, and manage this member directory."
        }
        "reviewer" => {
            "Reviewers participate in reviews and triage across the forge, the same as any authenticated member."
        }
        "member" => "Members can open issues, request reviews, and comment across the forge.",
        _ => {
            "This role is descriptive project metadata; only the maintainer/non-maintainer split changes what it can do."
        }
    }
}

/// The nav pane: every member in the directory, id + role at a glance, with
/// the signing key underneath so the identity is never hidden behind a
/// click.
async fn member_nav(cx: &Cx, items: &[Member], selected: Option<&str>) -> Result {
    let can_manage = can_manage_members(cx).await;
    view! { cx =>
        <div class="route-nav-header">
            <h2>"Trust"</h2>
            if can_manage {
                <a class="route-nav-action" href="/members/new">"+ add"</a>
            }
        </div>
        if items.is_empty() {
            <p class="route-nav-note">"No members found."</p>
        } else {
            for member in items {
                let row_class = if selected == Some(member.id.as_str()) {
                    "nav-row active"
                } else {
                    "nav-row"
                };
                <a class=(row_class) href=(format!("/members/{}", member.id))>
                    <span class="nav-row-top">
                        <span class="nav-row-title">(member.id.as_str())</span>
                        <span class="nav-row-role">(member.role.as_str())</span>
                    </span>
                    <span class="nav-row-sub">(member.signing_key.as_str())</span>
                </a>
            }
        }
    }
}

/// The detail pane for one identity: who they are, what their role means,
/// how much signed history is attributed to them, and the maintainer-only
/// actions. `orphaned` marks an id with attributions but no current
/// directory entry (a stale link) -- in that case there is no signing
/// key/role to show and no edit/remove action, just the banner and counts.
async fn member_detail(
    cx: &Cx,
    member: &Member,
    counts: &AttributionCounts,
    orphaned: bool,
) -> Result {
    let can_manage = can_manage_members(cx).await;
    view! { cx =>
        <article class="route-main-width-sm">
            if orphaned {
                <div class="trust-orphan-banner">"No current directory entry for " <code>(member.id.as_str())</code> " -- its signed history remains attributed as a plain string."</div>
            }
            <div class="trust-heading">
                <h1 class="trust-title">(member.id.as_str())</h1>
                if !orphaned {
                    <span class="trust-role-badge">(member.role.as_str())</span>
                }
            </div>
            if !orphaned {
                <p class="trust-key">(member.signing_key.as_str())</p>
                <p class="trust-role-explain">(role_explain(&member.role))</p>
            }
            <div class="trust-stats">
                <div class="trust-stat"><div class="trust-stat-label">"issues"</div><div class="trust-stat-value">(counts.issues)</div></div>
                <div class="trust-stat"><div class="trust-stat-label">"reviews"</div><div class="trust-stat-value">(counts.reviews)</div></div>
                <div class="trust-stat"><div class="trust-stat-label">"comments"</div><div class="trust-stat-value">(counts.comments)</div></div>
            </div>
            if orphaned {
                <p class="action-note">"This identity is not currently a member, so there is nothing to edit or remove here."</p>
            } else {
                <div class="detail-actions">
                    if can_manage {
                        <a class="button-link secondary" href=(format!("/members/{}/edit", member.id))>"Edit key / role"</a>
                        <form class="inline-form" action=(format!("/members/{}/delete", member.id)) method="post">
                            <button class="trust-danger-outline" type="submit">"Remove identity"</button>
                        </form>
                    } else {
                        <span class="action-note">"Member management is limited to maintainers."</span>
                    }
                </div>
                <p class="action-note">"Removing unlists the directory entry -- past attributions stay as plain strings."</p>
            }
        </article>
    }
}

#[page("/members")]
async fn members(cx: &Cx) -> Result {
    let data = with_repo(cx, load_members).await;

    let (nav, detail) = match data {
        Ok(members) => (
            member_nav(cx, &members, None).await?,
            (view! {
                <div class="route-main-width-sm">
                    <p class="eyebrow">"TRUST"</p>
                    <h2>"Select a member"</h2>
                    <p class="muted">"Choose an identity from the list to see its signing key, role, and attributed activity."</p>
                </div>
            })?,
        ),
        Err(error) => (
            (view! { <p class="error-text">(error)</p> })?,
            (view! { <p class="error-text">"Could not load members."</p> })?,
        ),
    };

    view! { route_shell(active: Tab::Trust, title: "Trust", nav_width: "19rem", nav: nav, detail: detail) }
}

#[page("/members/{member_id}")]
async fn member_detail_page(cx: &Cx) -> Result {
    let id = path_param::<MemberId>(cx)?.clone().to_string();
    let lookup_id = id.clone();
    let data = with_repo(cx, move |repo| {
        let members = load_members(repo)?;
        let member = members.iter().find(|m| m.id == lookup_id).cloned();
        let counts = count_attributions(repo, &lookup_id)?;
        Ok((members, member, counts))
    })
    .await;

    let (nav, detail) = match data {
        Ok((members, Some(member), counts)) => (
            member_nav(cx, &members, Some(member.id.as_str())).await?,
            member_detail(cx, &member, &counts, false).await?,
        ),
        Ok((members, None, counts)) if counts.issues + counts.reviews + counts.comments > 0 => {
            let orphan = Member {
                id: id.clone(),
                signing_key: String::new(),
                role: String::new(),
            };
            (
                member_nav(cx, &members, None).await?,
                member_detail(cx, &orphan, &counts, true).await?,
            )
        }
        Ok((members, None, _)) => (
            member_nav(cx, &members, None).await?,
            (view! { <div class="route-main-width-sm empty-panel"><h2>"Member not found"</h2><a href="/members">"Back to members"</a></div> })?,
        ),
        Err(error) => (
            (view! { <p class="error-text">(error)</p> })?,
            (view! { <p class="error-text">"Could not load member."</p> })?,
        ),
    };

    view! { route_shell(active: Tab::Trust, title: "Trust", nav_width: "19rem", nav: nav, detail: detail) }
}

#[page("/members/new")]
async fn member_new(cx: &Cx) -> Result {
    if !can_manage_members(cx).await {
        let content = (view! { <div class="empty-panel"><h2>"Maintainer access required"</h2><p class="muted">"Member management is intentionally limited to maintainers; the directory remains readable."</p><a class="button-link secondary" href="/members">"Back to members"</a></div> })?;
        return view! { shell(active: Some(Tab::Trust), title: "Add member", keyword: None, child: content) };
    }
    let form = MemberForm {
        signing_key: String::new(),
        role: "member".to_owned(),
    };
    let content = member_form(cx, "/members", &form, None).await?;
    view! { shell(active: Some(Tab::Trust), title: "Add member", keyword: None, child: content) }
}

#[page(POST "/members")]
async fn member_create(cx: &Cx, Form(input): Form<HashMap<String, String>>) -> Result {
    let authorization = authorization(cx).await?;
    let form = MemberForm::from_input(&input);
    if form.signing_key.trim().is_empty() || form.role.trim().is_empty() {
        let content = member_form(
            cx,
            "/members",
            &form,
            Some("Signing key and role are required."),
        )
        .await?;
        return view! { shell(active: Some(Tab::Trust), title: "Add member", keyword: None, child: content) };
    }
    if let Some(confirm) = confirm_step(
        cx,
        &input,
        "/members",
        "refs/meta/members/new",
        "/members",
        (view! { cx => "Add " <code>(form.signing_key.trim())</code> " as a new " (form.role.trim()) " member" })?,
    )
    .await?
    {
        return view! { shell(active: Some(Tab::Trust), title: "Confirm new member", keyword: None, child: confirm) };
    }
    let member = Member {
        id: String::new(),
        signing_key: form.signing_key.trim().to_owned(),
        role: form.role.trim().to_owned(),
    };
    let result = with_repo(cx, move |repo| {
        member.create_in_repo_as(repo, &authorization)
    })
    .await;
    match result {
        Ok(_id) => {
            let content = (view! { <section class="success-panel"><span class="success-icon">"✓"</span><p class="eyebrow">"MEMBER ADDED"</p><h2>"Trusted identity added"</h2><p class="muted">"This member is now available in the repository forge."</p><a class="button-link" href="/members">"View members"</a></section> })?;
            view! { shell(active: Some(Tab::Trust), title: "Member added", keyword: None, child: content) }
        }
        Err(error) => {
            if let Some(error) = authorization_error(&error) {
                return Err(error);
            }
            let content = member_form(cx, "/members", &form, Some(&error)).await?;
            view! { shell(active: Some(Tab::Trust), title: "Add member", keyword: None, child: content) }
        }
    }
}

#[page("/members/{member_id}/edit")]
async fn member_edit_page(cx: &Cx) -> Result {
    let id = path_param::<MemberId>(cx)?.clone();
    let data = with_repo(cx, move |repo| Member::load_from_repo(repo, &id)).await;
    let content = match data {
        Ok(Some(member)) => {
            if can_manage_members(cx).await {
                member_form(
                    cx,
                    &format!("/members/{}/edit", member.id),
                    &MemberForm::from_member(&member),
                    None,
                )
                .await?
            } else {
                (view! { <div class="empty-panel"><h2>"Maintainer access required"</h2><p class="muted">"The member directory is readable, but only maintainers can change identities or roles."</p><a class="button-link secondary" href="/members">"Back to members"</a></div> })?
            }
        }
        Ok(None) => {
            (view! { <div class="empty-panel"><h2>"Member not found"</h2><a href="/members">"Back to members"</a></div> })?
        }
        Err(error) => {
            (view! { <div class="error-panel"><strong>"Could not load member"</strong><p>(error)</p></div> })?
        }
    };
    view! { shell(active: Some(Tab::Trust), title: "Edit member", keyword: None, child: content) }
}

#[page(POST "/members/{member_id}/edit")]
async fn member_edit_submit(cx: &Cx, Form(input): Form<HashMap<String, String>>) -> Result {
    let authorization = authorization(cx).await?;
    let id = path_param::<MemberId>(cx)?.clone();
    let form = MemberForm::from_input(&input);
    if form.signing_key.trim().is_empty() || form.role.trim().is_empty() {
        let content = member_form(
            cx,
            &format!("/members/{id}/edit"),
            &form,
            Some("Signing key and role are required."),
        )
        .await?;
        return view! { shell(active: Some(Tab::Trust), title: "Edit member", keyword: None, child: content) };
    }
    if let Some(confirm) = confirm_step(
        cx,
        &input,
        &format!("/members/{id}/edit"),
        &format!("refs/meta/members/{id}"),
        "/members",
        (view! { cx => "Update member " <code>(id.as_str())</code> })?,
    )
    .await?
    {
        return view! { shell(active: Some(Tab::Trust), title: "Confirm member update", keyword: None, child: confirm) };
    }
    let lookup_id = id.clone();
    let result = with_repo(cx, move |repo| {
        let mut member = Member::load_from_repo(repo, &lookup_id)?.ok_or_else(|| {
            gix_forge::Error::InvalidTarget(format!("member `{lookup_id}` was not found"))
        })?;
        member.signing_key = form.signing_key.trim().to_owned();
        member.role = form.role.trim().to_owned();
        member
            .save_in_repo_as(repo, &authorization)
            .map(|_| member.id)
    })
    .await;
    match result {
        Ok(_) => {
            let content = (view! { <section class="success-panel"><span class="success-icon">"✓"</span><p class="eyebrow">"SAVED"</p><h2>"Member updated"</h2><a class="button-link" href="/members">"View members"</a></section> })?;
            view! { shell(active: Some(Tab::Trust), title: "Member updated", keyword: None, child: content) }
        }
        Err(error) => {
            if let Some(error) = authorization_error(&error) {
                return Err(error);
            }
            let content = (view! { <div class="error-panel"><strong>"Member could not be saved"</strong><p>(error)</p><a href=(format!("/members/{id}/edit"))>"Return to editor"</a></div> })?;
            view! { shell(active: Some(Tab::Trust), title: "Edit member", keyword: None, child: content) }
        }
    }
}

#[page(POST "/members/{member_id}/delete")]
async fn member_delete(cx: &Cx, Form(input): Form<HashMap<String, String>>) -> Result {
    let authorization = authorization(cx).await?;
    let id = path_param::<MemberId>(cx)?.clone();
    if let Some(confirm) = confirm_step(
        cx,
        &input,
        &format!("/members/{id}/delete"),
        &format!("refs/meta/members/{id}"),
        "/members",
        (view! { cx => "Remove member " <code>(id.as_str())</code> })?,
    )
    .await?
    {
        return view! { shell(active: Some(Tab::Trust), title: "Confirm member removal", keyword: None, child: confirm) };
    }
    let result = with_repo(cx, move |repo| Member::delete_as(repo, &id, &authorization)).await;
    let content = match result {
        Ok(true) => {
            (view! { <section class="success-panel"><span class="success-icon">"✓"</span><h2>"Member removed"</h2><a class="button-link" href="/members">"Return to members"</a></section> })?
        }
        Ok(false) => {
            (view! { <div class="empty-panel"><h2>"Member not found"</h2><a href="/members">"Return to members"</a></div> })?
        }
        Err(error) => {
            if let Some(error) = authorization_error(&error) {
                return Err(error);
            }
            (view! { <div class="error-panel"><strong>"Member could not be removed"</strong><p>(error)</p><a href="/members">"Return to members"</a></div> })?
        }
    };
    view! { shell(active: Some(Tab::Trust), title: "Remove member", keyword: None, child: content) }
}
