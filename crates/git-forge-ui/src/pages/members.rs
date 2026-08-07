use gix_forge::{EntityOps, Member};
use topcoat::{Result, context::Cx, router::page, view::view};

use crate::{
    pages::with_repo,
    shell::{Tab, shell},
};

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
        Ok(members) => {
            (view! {
                <section class="panel">
                    <div class="panel-heading">
                        <h2>"Members"</h2>
                        <span class="muted">(members.len())</span>
                    </div>
                    if members.is_empty() {
                        <p class="empty">"No members found."</p>
                    } else {
                        <div class="table-wrap">
                            <table>
                                <thead>
                                    <tr><th>"Signing key"</th><th>"Role"</th></tr>
                                </thead>
                                <tbody>
                                    for member in &members {
                                        <tr>
                                            <td><code>(member.signing_key.as_str())</code></td>
                                            <td><span class="tag">(member.role.as_str())</span></td>
                                        </tr>
                                    }
                                </tbody>
                            </table>
                        </div>
                    }
                </section>
            })?
        }
        Err(error) => {
            (view! {
                <div class="error-panel">
                    <strong>"Could not load members"</strong>
                    <p>(error)</p>
                </div>
            })?
        }
    };

    view! { shell(active: Tab::Members, title: "Members", child: content) }
}
