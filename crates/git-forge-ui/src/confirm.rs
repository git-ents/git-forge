//! The shared "signed action" confirm step every mutating POST handler
//! routes through: no mutation happens on the first submit, only a render of
//! the target ref, the identity that will be attributed, and a form that
//! re-submits the exact same fields with `confirmed=true` appended.
//!
//! There is no client-side JavaScript in this app, so "confirm" is a real
//! second HTTP round trip against the same POST endpoint, not a JS-toggled
//! overlay. A handler calls [`confirm_step`] after validating its input but
//! before performing the mutation: `Some(view)` means render the confirm
//! screen and return it immediately (wrapped in `shell(...)`); `None` means
//! the request already carries `confirmed=true` and the caller should
//! proceed with the real mutation exactly as before.
//!
//! # Example
//!
//! `issues.rs`'s `issue_comment` is wired in by the packet that owns that
//! file, not this one, but this is the shape of the call:
//!
//! ```ignore
//! #[page(POST "/issues/{issue_id}/comments")]
//! async fn issue_comment(cx: &Cx, Form(input): Form<HashMap<String, String>>) -> Result {
//!     let authorization = authorization(cx).await?;
//!     let id = path_param::<IssueId>(cx)?.clone();
//!     let body = input.get("body").cloned().unwrap_or_default();
//!     if body.trim().is_empty() {
//!         // ...existing validation, unchanged...
//!     }
//!     if let Some(confirm) = confirm_step(
//!         cx,
//!         &input,
//!         &format!("/issues/{id}/comments"),
//!         &format!("refs/meta/comments/line-{id}"),
//!         &format!("/issues/{id}"),
//!         (view! { cx => "Post a new comment on issue " <code>(id.as_str())</code> })?,
//!     )
//!     .await?
//!     {
//!         return view! { shell(active: Some(Tab::Issues), title: "Confirm comment", keyword: None, child: confirm) };
//!     }
//!     // ...proceed with the mutation exactly as today...
//! }
//! ```
//!
//! Other call sites pass whatever is semantically the real ref for that
//! action: `refs/meta/review/<id>` for a review verdict, `refs/tags/<tag>`
//! for a release claim, `refs/heads/<branch>` for a push.

use std::collections::HashMap;

use topcoat::{
    Result,
    context::Cx,
    view::{View, view},
};

use crate::auth::authenticated_member;

/// Whether `input` carries the `confirmed=true` field a confirm-screen
/// re-POST sets.
pub(crate) fn confirmed(input: &HashMap<String, String>) -> bool {
    input.get("confirmed").map(String::as_str) == Some("true")
}

/// Render the shared confirm screen for a mutating POST, or return `None`
/// once the caller should perform the real mutation.
///
/// `action_url` is the same endpoint the caller is already handling; the
/// confirm form re-POSTs every field already in `input` (as hidden inputs,
/// so nothing the caller parsed is lost) plus `confirmed=true` back to it.
/// `target_ref` is the ref-shaped string this action writes — see the
/// module docs for what "ref" means per action type, including the
/// `refs/meta/...` pseudo-ref convention for forge metadata that has no real
/// git ref. `cancel_href` sends the user back to the page the action
/// originated from. `summary` is a one-line description of what will happen.
///
/// The identity shown is the real authenticated member id, pulled here via
/// [`authenticated_member`]. Callers only reach this function after their
/// own `authorization(cx).await?` has already turned "no member" into an
/// auth error, so a member is always present.
pub(crate) async fn confirm_step(
    cx: &Cx,
    input: &HashMap<String, String>,
    action_url: &str,
    target_ref: &str,
    cancel_href: &str,
    summary: View,
) -> Result<Option<View>> {
    if confirmed(input) {
        return Ok(None);
    }
    let identity = authenticated_member(cx)
        .await
        .map(|member| member.id)
        .unwrap_or_default();
    let view = view! { cx =>
        <div class="confirm-scrim">
            <section class="confirm-modal">
                <p class="eyebrow">"CONFIRM"</p>
                <h2>"Confirm this action"</h2>
                <p class="confirm-summary">(summary)</p>
                <dl class="confirm-details">
                    <dt>"Ref"</dt><dd><code>(target_ref)</code></dd>
                    <dt>"Signed as"</dt><dd><code>(identity.as_str())</code></dd>
                </dl>
                <form class="confirm-form" action=(action_url) method="post">
                    for (name, value) in input.iter().filter(|(name, _)| name.as_str() != "confirmed") {
                        <input type="hidden" name=(name.as_str()) value=(value.as_str())>
                    }
                    <input type="hidden" name="confirmed" value="true">
                    <div class="form-actions">
                        <a class="button-link secondary" href=(cancel_href)>"Cancel"</a>
                        <button type="submit">"Confirm"</button>
                    </div>
                </form>
            </section>
        </div>
    }?;
    Ok(Some(view))
}
