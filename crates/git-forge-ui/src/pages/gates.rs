//! Live declarative gate evaluation and signed promotion.

use std::collections::HashMap;

use topcoat::{
    Result,
    context::Cx,
    router::{content::Form, page, path_param},
    view::view,
};

use crate::{
    auth::authorization,
    confirm::confirm_step,
    pages::with_repo,
    shell::{Tab, route_shell, shell},
};

#[path_param(error = bad_request)]
struct GateId(String);

async fn results(cx: &Cx) -> Result<Vec<gix_forge::GateResult>> {
    with_repo(cx, gix_forge::evaluate_gates)
        .await
        .map_err(|error| topcoat::Error::from(anyhow::anyhow!(error)))
}

fn selected<'a>(all: &'a [gix_forge::GateResult], id: &str) -> Option<&'a gix_forge::GateResult> {
    all.iter().find(|result| result.name == id)
}

async fn nav(cx: &Cx, all: &[gix_forge::GateResult], active: &str) -> Result {
    view! { cx =>
        <div class="route-nav-header">
            <h2>"Gates"</h2>
            <p class="route-nav-note">"derived by rules in refs/meta/rules — never stored"</p>
        </div>
        for result in all {
            let class = if result.name == active { "nav-row active" } else { "nav-row" };
            let verdict = if result.passed { "would pass" } else { "would fail" };
            <a class=(class) href=(format!("/gates/{}", result.name))>
                <div class="nav-row-top"><span class="nav-row-dot"></span><span class="nav-row-title">(result.name.as_str())</span></div>
                <span class="nav-row-sub">(result.subject.as_str())</span>
                <span class="nav-row-verdict">(verdict)</span>
            </a>
        }
    }
}

async fn detail(cx: &Cx, result: &gix_forge::GateResult) -> Result {
    let verdict = if result.passed {
        "would pass"
    } else {
        "would fail"
    };
    let class = if result.passed {
        "status ok gate-pill"
    } else {
        "status danger gate-pill"
    };
    view! { cx =>
        <div class="route-main-width-lg">
            <div class="gate-heading"><span class=(class)>(verdict)</span><h1>(result.name.as_str())</h1></div>
            <p class="gate-subline">"subject " <code>(result.subject.as_str())</code> " · evaluated in this page render"</p>
            <p class="eyebrow">"DERIVATION"</p>
            <div class="gate-trace">
                <div class="gate-trace-row"><span class="gate-trace-pred">(result.name.as_str())</span><span class="gate-trace-arrow">"⟸"</span><span class="gate-trace-value">(verdict)</span><span class="gate-trace-note">"Declarative rule evaluation completed; the query engine's module-predicate explanation is unavailable because qualified module predicates are not traceable yet."</span></div>
            </div>
            <div class="gate-info-grid">
                <div class="gate-info-panel"><h3>"Rule"</h3><dl class="facts"><dt>"Ref"</dt><dd><code class="oid">(format!("refs/meta/rules/{}", result.name))</code></dd><dt>"Tree"</dt><dd><code class="oid">(result.rule_ref.map_or_else(|| "missing".to_owned(), |id| id.to_string()))</code></dd></dl></div>
                <div class="gate-info-panel"><h3>"Input snapshot"</h3><dl class="facts"><dt>"State"</dt><dd>"fresh; no persisted result cache"</dd><dt>"Digest"</dt><dd><code class="oid">(result.input_digest.as_str())</code></dd><dt>"Keyed on"</dt><dd>"rule tip + claim/anchor/review refs"</dd></dl></div>
            </div>
            <div class="gate-actions">
                <form class="inline-form" action=(format!("/gates/{}/promote", result.name)) method="post"><button class="button-link secondary" type="submit">"Promote result to claim"</button></form>
            </div>
            <p class="action-note">"promotion is the only door from derived to authoritative"</p>
        </div>
    }
}

async fn render(cx: &Cx, id: &str) -> Result {
    let all = match results(cx).await {
        Ok(all) => all,
        Err(error) => {
            let nav = (view! { cx =>
                <div class="route-nav-header">
                    <h2>"Gates"</h2>
                    <p class="route-nav-note">"No installed rule set can evaluate this repository."</p>
                </div>
            })?;
            let detail = (view! { cx =>
                <div class="route-main-width-lg">
                    <h1>"Gates unavailable"</h1>
                    <p class="muted">"Gate evaluation failed: " (error.to_string())</p>
                </div>
            })?;
            return view! { cx => route_shell(active: Tab::Gates, title: "Gates", nav_width: "19rem", nav: nav, detail: detail) };
        }
    };
    let result = selected(&all, id)
        .or_else(|| all.first())
        .ok_or_else(|| topcoat::Error::from(anyhow::anyhow!("no gates are installed")))?;
    let nav_view = nav(cx, &all, &result.name).await?;
    let detail_view = detail(cx, result).await?;
    view! { cx => route_shell(active: Tab::Gates, title: "Gates", nav_width: "19rem", nav: nav_view, detail: detail_view) }
}

#[page("/gates")]
async fn gates(cx: &Cx) -> Result {
    render(cx, "merge-gate").await
}

#[page("/gates/{gate_id}")]
async fn gate_page(cx: &Cx) -> Result {
    let id = path_param::<GateId>(cx)?.clone();
    render(cx, id.as_str()).await
}

#[page(POST "/gates/{gate_id}/promote")]
async fn gate_promote(cx: &Cx, Form(input): Form<HashMap<String, String>>) -> Result {
    let authorization = authorization(cx).await?;
    let id = path_param::<GateId>(cx)?.clone();
    let all = results(cx).await?;
    let result = selected(&all, id.as_str())
        .ok_or_else(|| topcoat::Error::from(anyhow::anyhow!("unknown gate")))?
        .clone();
    let target = format!("refs/claims/gate/{}", result.subject);
    let summary = (view! { cx => "Sign the derived " <code>(result.name.as_str())</code> " verdict for " <code>(result.subject.as_str())</code> "." })?;
    if let Some(confirm) = confirm_step(
        cx,
        &input,
        &format!("/gates/{}/promote", result.name),
        &target,
        &format!("/gates/{}", result.name),
        summary,
    )
    .await?
    {
        return view! { shell(active: Some(Tab::Gates), title: "Confirm promotion", keyword: None, child: confirm) };
    }
    let promoted = with_repo(cx, move |repo| {
        gix_forge::promote_gate_as(repo, &authorization, &result)
    })
    .await
    .map_err(|error| topcoat::Error::from(anyhow::anyhow!(error)))?;
    let content = (view! { cx => <section class="success-panel"><span class="success-icon">"✓"</span><h2>"Gate verdict signed"</h2><p class="muted">"Claim " <code>(promoted.to_string())</code> " records the derived result."</p><a class="button-link" href=(format!("/gates/{}", id))>"Back to gate"</a></section> })?;
    view! { shell(active: Some(Tab::Gates), title: "Gate verdict signed", keyword: None, child: content) }
}
