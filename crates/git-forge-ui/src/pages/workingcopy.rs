use topcoat::{Result, context::Cx, router::page, view::view};

use crate::shell::{Tab, shell};

/// Stub route: content lands in packet E ("Releases + Working copy").
#[page("/workingcopy")]
async fn workingcopy(cx: &Cx) -> Result {
    let content = (view! { cx => <p class="empty">"Working copy is not built yet."</p> })?;
    view! { shell(active: Some(Tab::WorkingCopy), title: "Working copy", keyword: None, child: content) }
}
