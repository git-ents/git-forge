//! The theme toggle (non-negotiable #9: light/dark via `data-theme`,
//! defaulting to `prefers-color-scheme`). This app ships no client
//! JavaScript, so "click to cycle auto → light → dark → auto" is a plain
//! POST form (see `shell.rs`'s `theme_toggle`) that writes a `theme`
//! cookie server-side and redirects back to the page it was clicked from,
//! rather than a `localStorage` write plus a re-render in place.

use std::collections::HashMap;

use topcoat::{
    Result,
    context::Cx,
    cookie::{Cookie, Cookies, cookies},
    router::{content::Form, error::redirect, page},
};

#[page(POST "/theme")]
async fn theme_set(cx: &Cx, Form(input): Form<HashMap<String, String>>) -> Result {
    let mode: &'static str = match input.get("mode").map(String::as_str) {
        Some("light") => "light",
        Some("dark") => "dark",
        _ => "auto",
    };
    let from = input
        .get("from")
        .map(String::as_str)
        .filter(|value| value.starts_with('/'))
        .unwrap_or("/inbox");

    let jar = cookies(cx);
    if mode == "auto" {
        jar.remove(Cookie::build(("theme", "")).path("/").build());
    } else {
        jar.add(Cookie::build(("theme", mode)).path("/").build());
    }

    Err(redirect(from).into())
}
