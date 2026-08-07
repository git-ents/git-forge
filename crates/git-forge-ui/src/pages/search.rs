use std::collections::HashMap;

use topcoat::{
    Result,
    context::Cx,
    router::{content::Form, page},
    view::view,
};

use crate::{
    pages::with_repo,
    shell::{Tab, shell},
};

#[derive(Default)]
struct SearchForm {
    keyword: Option<String>,
    assignee: Option<String>,
    reviewer: Option<String>,
    requester: Option<String>,
}

async fn search_form(cx: &Cx, form: &SearchForm, hits: Option<&[(String, String)]>) -> Result {
    view! { cx =>
        <section class="panel query-panel">
            <form class="stack-form" action="/search" method="post">
                <label for="keyword">"Keyword"</label>
                <input id="keyword" name="keyword" value=(form.keyword.as_deref().unwrap_or_default()) placeholder="text in an issue or review">
                <label for="assignee">"Assignee"</label>
                <input id="assignee" name="assignee" value=(form.assignee.as_deref().unwrap_or_default())>
                <label for="reviewer">"Reviewer"</label>
                <input id="reviewer" name="reviewer" value=(form.reviewer.as_deref().unwrap_or_default())>
                <label for="requester">"Requester"</label>
                <input id="requester" name="requester" value=(form.requester.as_deref().unwrap_or_default())>
                <button type="submit">"Search"</button>
            </form>
        </section>
        if let Some(hits) = hits {
            <section class="panel query-results">
                <div class="panel-heading"><h2>"Matches"</h2><span class="muted">(hits.len())</span></div>
                if hits.is_empty() {
                    <p class="empty">"No matches."</p>
                } else {
                    <ul class="entity-list">
                        for (kind, id) in hits {
                            <li><a class="entity-link" href=(format!("/{kind}s/{id}"))><strong>(kind.as_str())</strong><span class="muted">(id.as_str())</span></a></li>
                        }
                    </ul>
                }
            </section>
        }
    }
}

#[page("/search")]
async fn search_page(cx: &Cx) -> Result {
    let form = SearchForm::default();
    let content = search_form(cx, &form, None).await?;
    view! { shell(active: Tab::Query, title: "Search", child: content) }
}

#[page(POST "/search")]
async fn search_submit(cx: &Cx, Form(input): Form<HashMap<String, String>>) -> Result {
    let form = SearchForm {
        keyword: input.get("keyword").cloned(),
        assignee: input.get("assignee").cloned(),
        reviewer: input.get("reviewer").cloned(),
        requester: input.get("requester").cloned(),
    };
    let assignee = form
        .assignee
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(str::to_owned);
    let reviewer = form
        .reviewer
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(str::to_owned);
    let requester = form
        .requester
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(str::to_owned);
    let keyword = form
        .keyword
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(str::to_owned);

    let result = with_repo(cx, move |repo| {
        Ok(gix_forge::search_find(
            repo,
            assignee.as_deref(),
            reviewer.as_deref(),
            requester.as_deref(),
            keyword.as_deref(),
        )?
        .into_iter()
        .map(|hit| (hit.kind.as_str().to_owned(), hit.id))
        .collect::<Vec<_>>())
    })
    .await;

    let content = match result {
        Ok(hits) => search_form(cx, &form, Some(&hits)).await?,
        Err(error) => {
            let form_view = search_form(cx, &form, None).await?;
            (view! { cx =>
                (form_view)
                <div class="error-panel"><strong>"Search failed"</strong><p>(error)</p></div>
            })?
        }
    };
    view! { shell(active: Tab::Query, title: "Search", child: content) }
}
