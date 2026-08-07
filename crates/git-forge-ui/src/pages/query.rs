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

struct QueryForm {
    goal: String,
    predicate: String,
    select: String,
}

async fn query_form(cx: &Cx, form: &QueryForm, rows: Option<&[Vec<String>]>) -> Result {
    view! { cx =>
        <section class="panel query-panel">
            <form class="stack-form" action="/query" method="post">
                <label for="goal">"Goal"</label>
                <input id="goal" name="goal" value=(form.goal.as_str()) placeholder="issue_status(Id, Status)">
                <label for="predicate">"Predicate"</label>
                <input id="predicate" name="predicate" value=(form.predicate.as_str()) placeholder="issue">
                <label for="select">"Select columns for a goal"</label>
                <input id="select" name="select" value=(form.select.as_str()) placeholder="Id Status">
                <p class="form-help">"Provide either a goal or a predicate. Goal columns are separated by spaces."</p>
                <button type="submit">"Run query"</button>
            </form>
        </section>
        if let Some(rows) = rows {
            <section class="panel query-results">
                <div class="panel-heading"><h2>"Results"</h2><span class="muted">(rows.len())</span></div>
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
        }
    }
}

#[page("/query")]
async fn query_page(cx: &Cx) -> Result {
    let form = QueryForm {
        goal: String::new(),
        predicate: String::new(),
        select: String::new(),
    };
    let content = query_form(cx, &form, None).await?;
    view! { shell(active: Tab::Query, title: "Query", keyword: None, child: content) }
}

#[page(POST "/query")]
async fn query_submit(cx: &Cx, Form(input): Form<HashMap<String, String>>) -> Result {
    let form = QueryForm {
        goal: input.get("goal").cloned().unwrap_or_default(),
        predicate: input.get("predicate").cloned().unwrap_or_default(),
        select: input.get("select").cloned().unwrap_or_default(),
    };
    let goal = (!form.goal.trim().is_empty()).then(|| form.goal.trim().to_owned());
    let predicate = (!form.predicate.trim().is_empty()).then(|| form.predicate.trim().to_owned());
    let select = form
        .select
        .split_whitespace()
        .map(str::to_owned)
        .collect::<Vec<_>>();

    let result: std::result::Result<Vec<Vec<String>>, String> = match (goal, predicate) {
        (Some(goal), None) if !select.is_empty() => {
            with_repo(cx, move |repo| {
                let columns = select.iter().map(String::as_str).collect::<Vec<_>>();
                Ok(gix_forge::query_goal(repo, &goal, &columns)?
                    .into_iter()
                    .map(|row| row.into_iter().map(|value| value.to_string()).collect())
                    .collect())
            })
            .await
        }
        (None, Some(predicate)) => {
            with_repo(cx, move |repo| {
                Ok(gix_forge::query_predicate(repo, &predicate, &[])?
                    .into_iter()
                    .map(|row| row.into_iter().map(|value| value.to_string()).collect())
                    .collect())
            })
            .await
        }
        (Some(_), None) => Err("A goal needs at least one selected column.".to_owned()),
        (Some(_), Some(_)) => Err("Provide a goal or a predicate, not both.".to_owned()),
        (None, None) => Err("Provide a goal or a predicate.".to_owned()),
    };

    let content = match result {
        Ok(rows) => query_form(cx, &form, Some(&rows)).await?,
        Err(error) => {
            let form_view = query_form(cx, &form, None).await?;
            (view! {
                (form_view)
                <div class="error-panel"><strong>"Query failed"</strong><p>(error)</p></div>
            })?
        }
    };
    view! { shell(active: Tab::Query, title: "Query", keyword: None, child: content) }
}
