use std::collections::HashMap;

use gix_forge::{EntityOps, Member};
use topcoat::{
    Result,
    context::Cx,
    router::{content::Form, page, path_param},
    view::view,
};

use crate::{
    auth::{authorization, authorization_error, can_manage_members},
    pages::with_repo,
    shell::{Tab, shell},
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

async fn member_list(cx: &Cx, items: &[Member]) -> Result {
    let can_manage = can_manage_members(cx).await;
    view! { cx =>
        <section class="panel">
            <div class="panel-heading"><div><h2>"Members"</h2><span class="muted">(items.len())</span></div>
                if can_manage {
                    <a class="button-link" href="/members/new">"Add member"</a>
                }
            </div>
            if items.is_empty() {
                <div class="empty-panel"><p>"No members found."</p>
                    if can_manage {
                        <a class="button-link" href="/members/new">"Add the first member"</a>
                    } else {
                        <p class="form-help">"Member management is limited to maintainers."</p>
                    }
                </div>
            } else {
                <div class="table-wrap">
                    <table>
                        <thead><tr><th>"Identity"</th><th>"Role"</th><th>"Actions"</th></tr></thead>
                        <tbody>
                            for member in items {
                                <tr>
                                    <td><a href=(format!("/members/{}/edit", member.id))><code>(member.signing_key.as_str())</code></a><small>(member.id.as_str())</small></td>
                                    <td><span class="tag">(member.role.as_str())</span></td>
                                    <td>
                                        if can_manage {
                                            <div class="table-actions"><a class="button-link secondary" href=(format!("/members/{}/edit", member.id))>"Edit"</a><form class="inline-form" action=(format!("/members/{}/delete", member.id)) method="post"><button class="danger-button" type="submit">"Remove"</button></form></div>
                                        } else {
                                            <span class="muted">"Maintainer only"</span>
                                        }
                                    </td>
                                </tr>
                            }
                        </tbody>
                    </table>
                </div>
            }
        </section>
    }
}

#[page("/members")]
async fn members(cx: &Cx) -> Result {
    let data = with_repo(cx, |repo| {
        let ids = Member::list(repo)?;
        ids.into_iter()
            .map(|id| Member::load_from_repo(repo, &id))
            .collect::<Result<Vec<_>, _>>()
            .map(|members| members.into_iter().flatten().collect::<Vec<Member>>())
    })
    .await;

    let content = match data {
        Ok(members) => member_list(cx, &members).await?,
        Err(error) => {
            (view! { <div class="error-panel"><strong>"Could not load members"</strong><p>(error)</p></div> })?
        }
    };

    view! { shell(active: Tab::Members, title: "Members", keyword: None, child: content) }
}

#[page("/members/new")]
async fn member_new(cx: &Cx) -> Result {
    if !can_manage_members(cx).await {
        let content = (view! { <div class="empty-panel"><h2>"Maintainer access required"</h2><p class="muted">"Member management is intentionally limited to maintainers; the directory remains readable."</p><a class="button-link secondary" href="/members">"Back to members"</a></div> })?;
        return view! { shell(active: Tab::Members, title: "Add member", keyword: None, child: content) };
    }
    let form = MemberForm {
        signing_key: String::new(),
        role: "member".to_owned(),
    };
    let content = member_form(cx, "/members", &form, None).await?;
    view! { shell(active: Tab::Members, title: "Add member", keyword: None, child: content) }
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
        return view! { shell(active: Tab::Members, title: "Add member", keyword: None, child: content) };
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
            view! { shell(active: Tab::Members, title: "Member added", keyword: None, child: content) }
        }
        Err(error) => {
            if let Some(error) = authorization_error(&error) {
                return Err(error);
            }
            let content = member_form(cx, "/members", &form, Some(&error)).await?;
            view! { shell(active: Tab::Members, title: "Add member", keyword: None, child: content) }
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
    view! { shell(active: Tab::Members, title: "Edit member", keyword: None, child: content) }
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
        return view! { shell(active: Tab::Members, title: "Edit member", keyword: None, child: content) };
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
            view! { shell(active: Tab::Members, title: "Member updated", keyword: None, child: content) }
        }
        Err(error) => {
            if let Some(error) = authorization_error(&error) {
                return Err(error);
            }
            let content = (view! { <div class="error-panel"><strong>"Member could not be saved"</strong><p>(error)</p><a href=(format!("/members/{id}/edit"))>"Return to editor"</a></div> })?;
            view! { shell(active: Tab::Members, title: "Edit member", keyword: None, child: content) }
        }
    }
}

#[page(POST "/members/{member_id}/delete")]
async fn member_delete(cx: &Cx) -> Result {
    let authorization = authorization(cx).await?;
    let id = path_param::<MemberId>(cx)?.clone();
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
    view! { shell(active: Tab::Members, title: "Remove member", keyword: None, child: content) }
}
