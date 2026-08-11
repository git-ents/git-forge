use std::collections::HashMap;

use topcoat::{
    Result,
    context::Cx,
    router::{content::Form, page},
    view::view,
};

use crate::{
    pages::with_repo,
    shell::shell,
};

#[derive(Default)]
struct SearchForm {
    keyword: Option<String>,
    assignee: Option<String>,
    reviewer: Option<String>,
    requester: Option<String>,
}

/// What a search submission produced: either the keyword/assignee/reviewer
/// find used by plain searches, or -- when the keyword starts with `?` -- the
/// rows of a bare predicate lookup, rendered the same way `/query` renders
/// them (`row.join(" · ")`). The compact top-bar box has no column-selection
/// UI, so a predicate name is all it accepts; `/query` remains the place for
/// goals and column selection.
enum SearchOutcome {
    Entities(Vec<(String, String)>),
    Predicate { predicate: String, rows: Vec<Vec<String>> },
}

async fn search_form(cx: &Cx, form: &SearchForm, outcome: Option<&SearchOutcome>) -> Result {
    view! { cx =>
        <section class="panel query-panel">
            <form class="stack-form" action="/search" method="post">
                <input type="hidden" name="keyword" value=(form.keyword.as_deref().unwrap_or_default())>
                <label for="assignee">"Assignee"</label>
                <input id="assignee" name="assignee" value=(form.assignee.as_deref().unwrap_or_default())>
                <label for="reviewer">"Reviewer"</label>
                <input id="reviewer" name="reviewer" value=(form.reviewer.as_deref().unwrap_or_default())>
                <label for="requester">"Requester"</label>
                <input id="requester" name="requester" value=(form.requester.as_deref().unwrap_or_default())>
                <button type="submit">"Search"</button>
            </form>
            <p class="form-help">"Prefix the top-bar search box with " <code>"?"</code> " to run a Datalog predicate by name (for example " <code>"?issue"</code> "). For goals and column selection, use the " <a href="/query">"full query console"</a> "."</p>
        </section>
        match outcome {
            Some(SearchOutcome::Entities(hits)) => {
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
            },
            Some(SearchOutcome::Predicate { predicate, rows }) => {
                <section class="panel query-results">
                    <div class="panel-heading"><h2>"?" (predicate.as_str())</h2><span class="muted">(rows.len())</span></div>
                    if rows.is_empty() {
                        <p class="empty">"No rows returned."</p>
                    } else {
                        <ol>
                            for row in rows {
                                <li><code>(row.join(" · "))</code></li>
                            }
                        </ol>
                    }
                </section>
            },
            None => "",
        }
    }
}

#[page("/search")]
async fn search_page(cx: &Cx) -> Result {
    let form = SearchForm::default();
    let content = search_form(cx, &form, None).await?;
    view! { shell(active: None, title: "Search", keyword: form.keyword.as_deref(), child: content) }
}

#[page(POST "/search")]
async fn search_submit(cx: &Cx, Form(input): Form<HashMap<String, String>>) -> Result {
    let form = SearchForm {
        keyword: input.get("keyword").cloned(),
        assignee: input.get("assignee").cloned(),
        reviewer: input.get("reviewer").cloned(),
        requester: input.get("requester").cloned(),
    };

    if let Some(predicate) = form
        .keyword
        .as_deref()
        .map(str::trim)
        .and_then(|keyword| keyword.strip_prefix('?'))
    {
        let predicate = predicate.trim().to_owned();
        let content = if predicate.is_empty() {
            let form_view = search_form(cx, &form, None).await?;
            (view! { cx =>
                (form_view)
                <div class="error-panel"><strong>"Search failed"</strong><p>"Provide a predicate name after " <code>"?"</code> "."</p></div>
            })?
        } else {
            let result = with_repo(cx, {
                let predicate = predicate.clone();
                move |repo| {
                    Ok(gix_forge::query_predicate(repo, &predicate, &[])?
                        .into_iter()
                        .map(|row| row.into_iter().map(|value| value.to_string()).collect())
                        .collect::<Vec<Vec<String>>>())
                }
            })
            .await;
            match result {
                Ok(rows) => {
                    search_form(cx, &form, Some(&SearchOutcome::Predicate { predicate, rows })).await?
                }
                Err(error) => {
                    let form_view = search_form(cx, &form, None).await?;
                    (view! { cx =>
                        (form_view)
                        <div class="error-panel"><strong>"Search failed"</strong><p>(error)</p></div>
                    })?
                }
            }
        };
        return view! { shell(active: None, title: "Search", keyword: form.keyword.as_deref(), child: content) };
    }

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
        Ok(hits) => search_form(cx, &form, Some(&SearchOutcome::Entities(hits))).await?,
        Err(error) => {
            let form_view = search_form(cx, &form, None).await?;
            (view! { cx =>
                (form_view)
                <div class="error-panel"><strong>"Search failed"</strong><p>(error)</p></div>
            })?
        }
    };
    view! { shell(active: None, title: "Search", keyword: form.keyword.as_deref(), child: content) }
}
