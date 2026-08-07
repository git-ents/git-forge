mod pages;
mod shell;

use std::sync::Arc;

use topcoat::{
    asset::{AssetBundle, RouterBuilderAssetExt},
    router::{Router, RouterBuilderDiscoverExt},
};

pub async fn build_router(repo: Arc<gix::ThreadSafeRepository>) -> Router {
    Router::builder()
        .discover()
        .app_context(repo)
        .assets(AssetBundle::load().expect("failed to load bundled assets"))
        .build()
}
