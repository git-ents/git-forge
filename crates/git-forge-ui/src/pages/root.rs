use topcoat::{Result, router::{error::redirect, page}};

/// The workspace has no dashboard; `/` exists only to send visitors
/// somewhere real.
#[page("/")]
async fn root() -> Result {
    Err(redirect("/tree").into())
}
