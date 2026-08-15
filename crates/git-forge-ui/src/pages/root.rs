use topcoat::{Result, router::{error::redirect, page}};

/// The workspace has no dashboard; `/` exists only to send visitors
/// somewhere real. Inbox is the app's front door (Handoff v5).
#[page("/")]
async fn root() -> Result {
    Err(redirect("/inbox").into())
}
