mod dashboard;
mod issues;
mod members;
mod query;
mod reviews;
mod search;

use topcoat::context::{Cx, app_context};

pub(crate) async fn with_repo<T, F>(cx: &Cx, operation: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce(&gix::Repository) -> Result<T, gix_forge::Error> + Send + 'static,
{
    let repo = app_context::<std::sync::Arc<gix::ThreadSafeRepository>>(cx).clone();
    tokio::task::spawn_blocking(move || {
        let repo = repo.to_thread_local();
        operation(&repo)
    })
    .await
    .map_err(|error| error.to_string())?
    .map_err(|error| error.to_string())
}
