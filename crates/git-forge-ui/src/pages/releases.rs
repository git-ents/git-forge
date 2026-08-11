use topcoat::{Result, context::Cx, router::page, view::view};

use crate::shell::{Tab, shell};

/// Stub route: content lands in packet E ("Releases + Working copy").
#[page("/releases")]
async fn releases(cx: &Cx) -> Result {
    let content = (view! { cx => <p class="empty">"Releases are not built yet."</p> })?;
    view! { shell(active: Some(Tab::Releases), title: "Releases", keyword: None, child: content) }
}
