//! Releases backed by annotated tags, typed release documents, and attest claims.

use std::collections::HashMap;

use gix_attest::{Target, target_key};
use gix_forge::{EntityOps, Release, ReleaseAttachment, ReleaseTarget};
use topcoat::{
    Result,
    context::Cx,
    router::{content::Form, page, path_param},
    view::view,
};

use crate::{
    auth::{authorization, authorization_error, can_create},
    confirm::confirm_step,
    pages::{csv_values, with_repo},
    shell::{Tab, route_shell, shell},
};

#[path_param(error = bad_request)]
struct ReleaseTag(String);

#[derive(Clone)]
struct ReleaseForm {
    target: String,
    body: String,
    attachments: String,
}

fn form_from_input(input: &HashMap<String, String>) -> ReleaseForm {
    ReleaseForm {
        target: input.get("target").cloned().unwrap_or_default(),
        body: input.get("body").cloned().unwrap_or_default(),
        attachments: input.get("attachments").cloned().unwrap_or_default(),
    }
}

async fn release_form(cx: &Cx, action: &str, form: &ReleaseForm, error: Option<&str>) -> Result {
    view! { cx =>
        <section class="form-layout">
            <div class="form-intro">
                <p class="eyebrow">"RELEASE DOCUMENT"</p>
                <h2>"Describe an annotated tag"</h2>
                <p class="muted">"The tag object id is immutable. This document stores only the release body and optional attachment references."</p>
            </div>
            if let Some(error) = error { <div class="error-panel"><strong>"Release could not be saved"</strong><p>(error)</p></div> }
            <form class="entity-form" action=(action) method="post">
                <fieldset>
                    <legend>"Release"</legend>
                    <label for="release-target">"Annotated tag target"</label>
                    <input id="release-target" name="target" value=(form.target.as_str()) placeholder="tag:v1.2.3@<full-tag-object-oid>" required="">
                    <label for="release-body">"Body"</label>
                    <textarea id="release-body" name="body" rows="10" placeholder="What does this release contain?">(form.body.as_str())</textarea>
                    <label for="release-attachments">"Attachments " <span class="muted">"(comma-separated name@blob-oid values)"</span></label>
                    <input id="release-attachments" name="attachments" value=(form.attachments.as_str()) placeholder="SHA256SUMS@<blob-oid>">
                </fieldset>
                <div class="form-actions"><a class="button-link secondary" href="/releases">"Cancel"</a><button type="submit">"Save release"</button></div>
            </form>
        </section>
    }
}

fn attachment_text(release: &Release) -> String {
    release
        .attachments
        .iter()
        .map(|a| format!("{}@{}", a.name, a.oid))
        .collect::<Vec<_>>()
        .join(", ")
}

fn parse_attachments(value: &str) -> Result<Vec<ReleaseAttachment>, String> {
    csv_values(value)
        .into_iter()
        .map(|item| {
            let (name, oid) = item
                .rsplit_once('@')
                .ok_or_else(|| format!("attachment `{item}` must be name@blob-oid"))?;
            gix::ObjectId::from_hex(oid.as_bytes())
                .map_err(|_| format!("attachment `{item}` has an invalid object id"))?;
            Ok(ReleaseAttachment {
                name: name.to_owned(),
                oid: oid.to_ascii_lowercase(),
            })
        })
        .collect()
}

type ReleaseRow = (ReleaseTarget, Option<Release>, Vec<gix_forge::ReleaseClaim>);

fn load_rows(repo: &gix::Repository) -> std::result::Result<Vec<ReleaseRow>, gix_forge::Error> {
    Release::annotated_tags(repo)?
        .into_iter()
        .map(|target| {
            let release = Release::load_from_repo(repo, &target.tag)?;
            let claims = Release::claims_for_tag(repo, &target).unwrap_or_default();
            Ok((target, release, claims))
        })
        .collect()
}

async fn release_nav(cx: &Cx, rows: &[ReleaseRow], selected: Option<&str>) -> Result {
    let can_create = can_create::<Release>(cx).await;
    view! { cx =>
        <div class="route-nav-header"><h2>"Releases"</h2>{if can_create { <a class="route-nav-action" href="/releases/new">"+ document"</a> }}</div>
        <p class="route-nav-note">"annotated tags with typed bodies and signed claims"</p>
        if rows.is_empty() { <div class="empty-panel"><p>"No annotated tags found."</p></div> }
        for (target, release, claims) in rows {
            let active = selected == Some(target.tag.as_str());
            let row_class = if active { "nav-row active" } else { "nav-row" };
            let valid = claims.iter().filter(|claim| claim.valid && claim.revoked.is_none()).count();
            let label = if claims.is_empty() { "no claims".to_owned() } else { format!("{valid}/{} valid", claims.len()) };
            <a class=(row_class) href=(format!("/releases/{}", target.tag))>
                <div class="nav-row-top"><span class="anchor-kind oid">"tag"</span><span class="nav-row-status neutral">if release.is_some() { "documented" } else { "undocumented" }</span></div>
                <span class="nav-row-title">(target.tag.as_str())</span><span class="nav-row-sub">(target.oid.as_str()) " · " (label)</span>
            </a>
        }
    }
}

async fn release_detail(
    cx: &Cx,
    target: &ReleaseTarget,
    release: Option<&Release>,
    claims: &[gix_forge::ReleaseClaim],
) -> Result {
    let claim_ref = target_key(&Target {
        kind: "tag".into(),
        id: target.object_id().into(),
    })
    .map(|id| format!("refs/claims/{id}"))
    .unwrap_or_else(|_| "refs/claims/<target-key>".into());
    view! { cx =>
        <article class="route-main-width-md">
            <div class="detail-meta"><div class="detail-meta-group"><span class="status neutral">"ANNOTATED TAG"</span><strong class="oid">(target.tag.as_str())</strong></div><span class="muted">"tag object " <span class="oid">(target.oid.as_str())</span></span></div>
            if let Some(release) = release {
                <div class="body-copy">(release.body.as_str())</div>
                <h3>"Attachments"</h3>
                if release.attachments.is_empty() { <p class="empty">"No attachments recorded."</p> } else { <div class="attachment-list">for attachment in &release.attachments { <div class="attachment-row"><span>(attachment.name.as_str())</span><span class="oid">(attachment.oid.as_str())</span></div> }</div> }
                <div class="detail-actions"><a class="button-link secondary" href=(format!("/releases/{}/edit", target.tag))>"Edit document"</a></div>
            } else { <div class="empty-panel"><p>"No release document is recorded for this annotated tag."</p><a class="button-link" href=(format!("/releases/new?target={}", target))>"Create document"</a></div> }
            <h3>"Signed claims"</h3><p class="muted">"Claims target the immutable annotated tag object. Validity includes cryptographic verification; revocations and key rotations are resolved by gix-attest."</p>
            if claims.is_empty() { <p class="empty panel-block-body">"No signed claims yet."</p> } else { <ul class="claim-list panel-block">for claim in claims { let status = if claim.revoked.is_some() { "revoked" } else if claim.valid { "valid" } else { "invalid" }; let class = if status == "valid" { "ok" } else { "danger" }; <li class="claim-row"><strong>(claim.signer.as_str())</strong><span class=(format!("claim-verdict {class}"))>(status)</span><span class="muted">(claim.date.as_str())</span><span class="claim-key">"key " (claim.key.to_string())</span></li> }</ul> }
            <p class="form-help">"claim ref " <code>(claim_ref.as_str())</code></p>
            <form class="inline-form" action=(format!("/releases/{}/publish", target.tag)) method="post"><button type="submit">"Sign claim on this tag"</button></form>
        </article>
    }
}

#[page("/releases")]
async fn releases(cx: &Cx) -> Result {
    let rows = with_repo(cx, load_rows)
        .await
        .map_err(|error| topcoat::Error::from(anyhow::anyhow!(error)))?;
    let nav = release_nav(cx, &rows, None).await?;
    let detail = (view! { <div class="placeholder-detail"><p class="eyebrow">"RELEASE DETAIL"</p><h2>"Select an annotated tag"</h2><p class="muted">"Only repository tags and stored release documents appear here."</p></div> })?;
    view! { route_shell(active: Tab::Releases, title: "Releases", nav_width: "21rem", nav: nav, detail: detail) }
}

#[page("/releases/{release_tag}")]
async fn release_page(cx: &Cx) -> Result {
    let tag = path_param::<ReleaseTag>(cx)?.clone();
    let rows = with_repo(cx, load_rows)
        .await
        .map_err(|error| topcoat::Error::from(anyhow::anyhow!(error)))?;
    let nav = release_nav(cx, &rows, Some(tag.as_str())).await?;
    let detail = match rows.iter().find(|(target, _, _)| target.tag == tag) {
        Some((target, release, claims)) => {
            release_detail(cx, target, release.as_ref(), claims).await?
        }
        None => (view! { <div class="empty-panel"><h2>"Annotated tag not found"</h2><a href="/releases">"Back to releases"</a></div> })?,
    };
    view! { route_shell(active: Tab::Releases, title: "Release", nav_width: "21rem", nav: nav, detail: detail) }
}

#[page("/releases/new")]
async fn release_new(cx: &Cx) -> Result {
    let content = release_form(
        cx,
        "/releases",
        &ReleaseForm {
            target: String::new(),
            body: String::new(),
            attachments: String::new(),
        },
        None,
    )
    .await?;
    view! { shell(active: Some(Tab::Releases), title: "New release", keyword: None, child: content) }
}

#[page(POST "/releases")]
async fn release_create(cx: &Cx, Form(input): Form<HashMap<String, String>>) -> Result {
    let authorization = authorization(cx).await?;
    let form = form_from_input(&input);
    let form_for_repo = form.clone();
    let result = with_repo(cx, move |repo| {
        let target = ReleaseTarget::parse(form_for_repo.target.trim())?;
        if !Release::annotated_tags(repo)?.contains(&target) {
            return Err(gix_forge::Error::InvalidTarget(
                "target is not an annotated tag in this repository".into(),
            ));
        }
        let attachments = parse_attachments(&form_for_repo.attachments)
            .map_err(gix_forge::Error::InvalidTarget)?;
        let release = Release {
            id: target.tag.clone(),
            target,
            body: form_for_repo.body,
            attachments,
            edit: None,
        };
        release
            .create_in_repo_as(repo, &authorization)
            .map(|_| release.id)
    })
    .await;
    match result {
        Ok(tag) => {
            view! { shell(active: Some(Tab::Releases), title: "Release saved", keyword: None, child: (view! { <div class="success-panel"><h2>"Release document saved"</h2><a class="button-link" href=(format!("/releases/{tag}"))>"Open release"</a></div> })?) }
        }
        Err(error) => {
            let content = release_form(cx, "/releases", &form, Some(&error)).await?;
            view! { shell(active: Some(Tab::Releases), title: "Release failed", keyword: None, child: content) }
        }
    }
}

#[page("/releases/{release_tag}/edit")]
async fn release_edit(cx: &Cx) -> Result {
    let tag = path_param::<ReleaseTag>(cx)?.clone();
    let tag_for_repo = tag.clone();
    let release = with_repo(cx, move |repo| Release::load_from_repo(repo, &tag_for_repo))
        .await
        .map_err(|error| topcoat::Error::from(anyhow::anyhow!(error)))?
        .ok_or_else(|| {
            topcoat::Error::from(anyhow::anyhow!(topcoat::router::error::not_found()))
        })?;
    let attachments = attachment_text(&release);
    let form = ReleaseForm {
        target: release.target.to_string(),
        body: release.body,
        attachments,
    };
    let content = release_form(cx, &format!("/releases/{tag}/edit"), &form, None).await?;
    view! { shell(active: Some(Tab::Releases), title: "Edit release", keyword: None, child: content) }
}

#[page(POST "/releases/{release_tag}/edit")]
async fn release_update(cx: &Cx, Form(input): Form<HashMap<String, String>>) -> Result {
    let authorization = authorization(cx).await?;
    let tag = path_param::<ReleaseTag>(cx)?.clone();
    let form = form_from_input(&input);
    let form_for_repo = form.clone();
    let tag_for_repo = tag.clone();
    let result = with_repo(cx, move |repo| {
        let target = ReleaseTarget::parse(form_for_repo.target.trim())?;
        if target.tag != tag_for_repo || !Release::annotated_tags(repo)?.contains(&target) {
            return Err(gix_forge::Error::InvalidTarget(
                "release target must remain the same annotated tag".into(),
            ));
        }
        Release {
            id: tag_for_repo.clone(),
            target,
            body: form_for_repo.body,
            attachments: parse_attachments(&form_for_repo.attachments)
                .map_err(gix_forge::Error::InvalidTarget)?,
            edit: None,
        }
        .save_in_repo_as(repo, &authorization)
        .map(|_| tag_for_repo)
    })
    .await;
    match result {
        Ok(tag) => {
            view! { shell(active: Some(Tab::Releases), title: "Release saved", keyword: None, child: (view! { <div class="success-panel"><h2>"Release document saved"</h2><a class="button-link" href=(format!("/releases/{tag}"))>"Open release"</a></div> })?) }
        }
        Err(error) => {
            let content =
                release_form(cx, &format!("/releases/{tag}/edit"), &form, Some(&error)).await?;
            view! { shell(active: Some(Tab::Releases), title: "Edit failed", keyword: None, child: content) }
        }
    }
}

#[page(POST "/releases/{release_tag}/publish")]
async fn release_publish(cx: &Cx, Form(input): Form<HashMap<String, String>>) -> Result {
    let authorization = authorization(cx).await?;
    let tag = path_param::<ReleaseTag>(cx)?.clone();
    let tag_for_repo = tag.clone();
    let target = with_repo(cx, move |repo| {
        Release::annotated_tags(repo)?
            .into_iter()
            .find(|target| target.tag == tag_for_repo)
            .ok_or_else(|| gix_forge::Error::InvalidTarget("annotated tag not found".into()))
    })
    .await
    .map_err(|error| topcoat::Error::from(anyhow::anyhow!(error)))?;
    let ref_name = target_key(&Target {
        kind: "tag".into(),
        id: target.object_id().into(),
    })
    .map(|id| format!("refs/claims/{id}"))
    .unwrap_or_else(|_| "refs/claims/<target-key>".into());
    if let Some(confirm) = confirm_step(
        cx,
        &input,
        &format!("/releases/{}/publish", target.tag),
        &ref_name,
        &format!("/releases/{}", target.tag),
        (view! { cx => "Sign a release claim for " <code>(target.tag.as_str())</code> })?,
    )
    .await?
    {
        return view! { shell(active: Some(Tab::Releases), title: "Confirm claim signature", keyword: None, child: confirm) };
    }
    let result = with_repo(cx, move |repo| {
        Release::publish_claim_as(repo, &authorization, &target)
    })
    .await;
    match result {
        Ok(_) => {
            view! { shell(active: Some(Tab::Releases), title: "Claim signed", keyword: None, child: (view! { <div class="success-panel"><h2>"Release claim signed"</h2><a class="button-link" href=(format!("/releases/{}", tag))>"Back to release"</a></div> })?) }
        }
        Err(error) => {
            if let Some(error) = authorization_error(&error) {
                return Err(error);
            }
            view! { shell(active: Some(Tab::Releases), title: "Claim failed", keyword: None, child: (view! { <div class="error-panel"><strong>"Release claim could not be signed"</strong><p>(error)</p></div> })?) }
        }
    }
}
