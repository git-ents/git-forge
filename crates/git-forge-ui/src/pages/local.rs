//! The "Local" ambient strip (Handoff v5): always visible at the foot of
//! the shell, showing how uncommitted local work would affect the signed
//! record if pushed. Not a route -- [`local_strip`] is mounted directly by
//! `shell.rs` on every page, the same way `palette_mount` is.
//!
//! There is no dirty-worktree, unpushed-forge-ref-counting, or drift/gate
//! backend in this codebase yet (see `gix-forge`'s query surface: nothing
//! computes drift or gate state today). Real per-file added/deleted line
//! counts and forge-ref push accounting are more than a "cheap nice-to-have"
//! wire-up, so the counts and warnings below are sample content previewing
//! the intended design, matching the precedent set by the former
//! `workingcopy.rs` board. The one real touch is `headOid`, read straight
//! off the repository. "Push forge refs" is a no-op that only demonstrates
//! the shared confirm step; it does not read, sign, or push anything real.

use std::collections::HashMap;

use topcoat::{
    Result,
    context::Cx,
    router::{content::Form, page},
    view::view,
};

use crate::{auth::authorization, confirm::confirm_step, pages::with_repo, shell::shell};

const DIRTY_COUNT: u32 = 3;
const UNPUSHED_COMMENTS: u32 = 4;
const UNPUSHED_CLAIMS: u32 = 1;
const DRIFT_WARNING: &str = "1 binding fell below the rule threshold since HEAD moved -- see Annotations";
const GATE_WARNING: &str = "merge gate would go red on push -- see Gates";
const ENGINE_VERSION: &str = env!("CARGO_PKG_VERSION");

async fn head_oid(cx: &Cx) -> String {
    with_repo(cx, |repo| {
        Ok(repo
            .head_id()
            .map(|id| id.to_string()[..7].to_owned())
            .unwrap_or_else(|_| "unborn".to_owned()))
    })
    .await
    .unwrap_or_else(|_| "unknown".to_owned())
}

/// Mounted once per page by `shell`/`route_shell`. Zero client JavaScript,
/// so the expand/collapse behavior the design mock drives with
/// `toggleLocal` state is a CSS `:target` panel instead -- the same
/// fragment-driven pattern `palette.rs` already uses for its overlay.
pub(crate) async fn local_strip(cx: &Cx) -> Result {
    let head = head_oid(cx).await;
    view! { cx =>
        <div class="local-strip">
            <div id="local" class="local-panel">
                <p class="local-panel-heading">"Local — what your uncommitted work does to the record"</p>
                <div class="local-panel-body">
                    <p>(DIRTY_COUNT) " dirty files at HEAD " <code class="oid">(head.as_str())</code></p>
                    <p>(UNPUSHED_COMMENTS) " comments and " (UNPUSHED_CLAIMS) " signed claim(s) exist only on this machine"</p>
                    <p class="warn">(DRIFT_WARNING)</p>
                    <p class="danger">(GATE_WARNING)</p>
                </div>
                <form class="local-push-form" action="/local/push" method="post">
                    <button type="submit">"Push forge refs"</button>
                </form>
                <a class="local-panel-close" href="#">"Close"</a>
            </div>
            <div class="local-bar">
                <a class="local-bar-item" href="#local">
                    <span class="status-dot ok"></span>"HEAD " <code class="oid">(head.as_str())</code>
                </a>
                <span class="local-bar-item warn">(DIRTY_COUNT) " dirty"</span>
                <a class="local-bar-item warn local-bar-link" href="#local">(UNPUSHED_COMMENTS + UNPUSHED_CLAIMS) " forge refs unpushed"</a>
                <span class="local-bar-item local-bar-engine">"engine v" (ENGINE_VERSION)</span>
            </div>
        </div>
    }
}

#[page(POST "/local/push")]
async fn local_push(cx: &Cx, Form(input): Form<HashMap<String, String>>) -> Result {
    authorization(cx).await?;
    let head = head_oid(cx).await;
    let summary = (view! { cx =>
        "Preview only: this records nothing. There is no push or signing backend yet, so confirming here would push "
        (UNPUSHED_COMMENTS) " comment(s) and " (UNPUSHED_CLAIMS) " signed claim(s) from " <code>(head.as_str())</code>
        " if it were real."
    })?;
    if let Some(confirm) = confirm_step(
        cx,
        &input,
        "/local/push",
        "refs/forge/**",
        "/inbox",
        summary,
    )
    .await?
    {
        return view! { shell(active: None, title: "Confirm push", keyword: None, child: confirm) };
    }
    let content = (view! { cx =>
        <section class="success-panel">
            <span class="success-icon">"✓"</span>
            <p class="eyebrow">"PREVIEW"</p>
            <h2>"Push confirmed (no-op)"</h2>
            <p class="muted">"This UI has no push or signing backend yet, so nothing was actually signed or pushed."</p>
            <a class="button-link" href="/inbox">"Back to Inbox"</a>
        </section>
    })?;
    view! { shell(active: None, title: "Push (preview)", keyword: None, child: content) }
}
