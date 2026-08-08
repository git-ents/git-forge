pub mod auth;
mod pages;
pub mod render;
mod shell;
pub mod tree;

use std::{fs, sync::Arc};

use topcoat::{
    asset::{AssetBundle, RouterBuilderAssetExt},
    router::{Router, RouterBuilderDiscoverExt},
};
use topcoat_asset::{Bundler, BundlerConfig};

pub async fn build_router(
    repo: Arc<gix::ThreadSafeRepository>,
    member_id: Option<String>,
) -> Router {
    let executable = std::env::current_exe()
        .expect("failed to locate the current executable for asset bundling");
    let assets_dir = executable
        .parent()
        .expect("current executable has no parent directory for asset bundling")
        .join("assets");
    let binary = fs::read(&executable).unwrap_or_else(|error| {
        panic!(
            "failed to read executable {} for asset bundling: {error}",
            executable.display()
        )
    });

    let config = BundlerConfig::new().cache_dir(assets_dir.join(".cache"));
    Bundler::new(&config)
        .bundle(&binary, &assets_dir)
        .unwrap_or_else(|error| {
            panic!(
                "failed to bundle assets from {} into {}: {error}",
                executable.display(),
                assets_dir.display()
            )
        });
    let assets = AssetBundle::load_dir(&assets_dir).unwrap_or_else(|error| {
        panic!(
            "failed to load the freshly bundled assets from {}: {error}",
            assets_dir.display()
        )
    });

    Router::builder()
        .discover()
        .app_context(repo)
        .app_context(member_id)
        .assets(assets)
        .build()
}
