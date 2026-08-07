use std::panic::{AssertUnwindSafe, catch_unwind};

use topcoat::{
    Result,
    context::Cx,
    router::{page, query_params},
    view::{Unescaped, View, view},
};

use crate::{
    pages::with_repo,
    render::{highlight_file, render_asciidoc, render_markdown},
    shell::{Tab, shell},
    tree::{self, Browse, BrowseView, File},
};

#[query_params(error = bad_request)]
struct TreeQuery {
    #[serde(rename = "ref")]
    reference: Option<String>,
    path: Option<String>,
    view: Option<String>,
}

#[page("/tree")]
async fn files(cx: &Cx) -> Result {
    let query = query_params::<TreeQuery>(cx)?;
    let requested_ref = query.reference.clone();
    let requested_path = query.path.clone();
    let source = query.view.as_deref() == Some("source");
    let browse = with_repo(cx, move |repo| {
        tree::load(repo, requested_ref.as_deref(), requested_path.as_deref())
            .map_err(|error| gix_forge::Error::QueryRules(error.to_string()))
    })
    .await;

    let content = match browse {
        Ok(browse) => browse_view(cx, &browse, source).await?,
        Err(error) => error_panel(cx, "Could not load repository tree", &error).await?,
    };

    view! { shell(active: Tab::Files, title: "Files", child: content) }
}

async fn browse_view(cx: &Cx, browse: &Browse, source: bool) -> Result {
    match &browse.view {
        BrowseView::Directory { object_id, entries } => {
            directory_view(cx, browse, *object_id, entries).await
        }
        BrowseView::File(file) => file_view(cx, browse, file, source).await,
    }
}

async fn directory_view(
    cx: &Cx,
    browse: &Browse,
    object_id: gix::ObjectId,
    entries: &[tree::Entry],
) -> Result {
    let current_path = if browse.path.is_empty() {
        "/"
    } else {
        browse.path.as_str()
    };
    view! { cx =>
        <section class="panel tree-panel">
            <nav class="breadcrumbs" aria-label="Breadcrumb">
                for breadcrumb in &browse.breadcrumbs {
                    <a href=(tree_href(&browse.reference.selector, &breadcrumb.path, None))>
                        (breadcrumb.display.as_str())
                    </a>
                }
            </nav>
            <div class="detail-meta">
                <span class="status">"directory"</span>
                <span class="muted">"ref: " (browse.reference.name.as_str())</span>
                <span class="muted">"path: " (current_path)</span>
                <span class="muted">"tree: " (object_id.to_string())</span>
            </div>
            if entries.is_empty() {
                <p class="empty">"This directory is empty."</p>
            } else {
                <ul class="entity-list file-list">
                    for entry in entries {
                        <li>
                            <a class="entity-link" href=(tree_href(&browse.reference.selector, &entry.path, None))>
                                <span>
                                    <strong>(entry.display_name.as_str())</strong>
                                    <small>(entry_kind_label(entry.kind))</small>
                                </span>
                                <span class="muted">
                                    if let Some(size) = entry.size {
                                        (format!("{size} bytes"))
                                    } else {
                                        "—"
                                    }
                                    " · "
                                    (entry.object_id.to_string())
                                </span>
                            </a>
                        </li>
                    }
                </ul>
            }
        </section>
    }
}

async fn file_view(cx: &Cx, browse: &Browse, file: &File, source: bool) -> Result {
    let current_path = browse.path.as_str();
    let body: View = if source {
        (view! { cx =>
            <pre class="source-view"><code>(file.text.as_str())</code></pre>
        })?
    } else {
        match render_preview(current_path, &file.text) {
            Ok(Preview::Html(html)) => {
                (view! { cx =>
                    <div class="rendered-file">(Unescaped::new_unchecked(html))</div>
                })?
            }
            Ok(Preview::Source(source)) => {
                (view! { cx =>
                    <pre class="source-view"><code>(source.as_str())</code></pre>
                })?
            }
            Err(error) => {
                (view! { cx =>
                    <div class="error-panel">
                        <strong>"Could not render this file"</strong>
                        <p>(error.as_str())</p>
                        <p>"Showing escaped source instead."</p>
                        <pre class="source-view"><code>(file.text.as_str())</code></pre>
                    </div>
                })?
            }
        }
    };

    view! { cx =>
        <section class="panel tree-panel">
            <nav class="breadcrumbs" aria-label="Breadcrumb">
                for breadcrumb in &browse.breadcrumbs {
                    <a href=(tree_href(&browse.reference.selector, &breadcrumb.path, None))>
                        (breadcrumb.display.as_str())
                    </a>
                }
            </nav>
            <div class="detail-meta">
                <span class="status">"file"</span>
                <span class="muted">"ref: " (browse.reference.name.as_str())</span>
                <span class="muted">"path: " (current_path)</span>
                <span class="muted">(format!("{} bytes", file.size))</span>
                <span class="muted">"blob: " (file.object_id.to_string())</span>
            </div>
            <div class="view-toggle" aria-label="File view">
                <a href=(tree_href(&browse.reference.selector, &browse.route_path, Some("preview")))>
                    "Preview"
                </a>
                <a href=(tree_href(&browse.reference.selector, &browse.route_path, Some("source")))>
                    "Source"
                </a>
            </div>
            (body)
        </section>
    }
}

async fn error_panel(cx: &Cx, title: &str, error: &str) -> Result {
    view! { cx =>
        <div class="error-panel">
            <strong>(title)</strong>
            <p>(error)</p>
            <p>"Try another ref or path."</p>
        </div>
    }
}

enum Preview {
    Html(String),
    Source(String),
}

fn render_preview(path: &str, source: &str) -> std::result::Result<Preview, String> {
    catch_unwind(AssertUnwindSafe(|| {
        if is_markdown(path) {
            Preview::Html(render_markdown(source))
        } else if is_asciidoc(path) {
            Preview::Html(render_asciidoc(source))
        } else if arborium::detect_language(path).is_some() {
            Preview::Html(highlight_file(path, source))
        } else {
            Preview::Source(source.to_owned())
        }
    }))
    .map_err(|_| format!("The renderer failed for {path:?}."))
}

fn is_markdown(path: &str) -> bool {
    matches!(
        path.rsplit('/').next().and_then(|name| name.rsplit_once('.')),
        Some((_, extension))
            if extension.eq_ignore_ascii_case("md")
                || extension.eq_ignore_ascii_case("markdown")
                || extension.eq_ignore_ascii_case("mdown")
                || extension.eq_ignore_ascii_case("mkdn")
    )
}

fn is_asciidoc(path: &str) -> bool {
    matches!(
        path.rsplit('/').next().and_then(|name| name.rsplit_once('.')),
        Some((_, extension))
            if extension.eq_ignore_ascii_case("adoc")
                || extension.eq_ignore_ascii_case("asciidoc")
                || extension.eq_ignore_ascii_case("ad")
                || extension.eq_ignore_ascii_case("asc")
    )
}

fn entry_kind_label(kind: gix::objs::tree::EntryKind) -> &'static str {
    match kind {
        gix::objs::tree::EntryKind::Tree => "directory",
        gix::objs::tree::EntryKind::Blob | gix::objs::tree::EntryKind::BlobExecutable => "file",
        _ => "special entry",
    }
}

fn tree_href(reference: &str, route_path: &str, view: Option<&str>) -> String {
    let mut href = format!(
        "/tree?ref={reference}&path={}",
        encode_query_path(route_path)
    );
    if let Some(view) = view {
        href.push_str("&view=");
        href.push_str(view);
    }
    href
}

fn encode_query_path(route_path: &str) -> String {
    percent_encode(&percent_decode(route_path))
}

fn percent_decode(value: &str) -> Vec<u8> {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%'
            && index + 2 < bytes.len()
            && let (Some(high), Some(low)) =
                (hex_value(bytes[index + 1]), hex_value(bytes[index + 2]))
        {
            decoded.push((high << 4) | low);
            index += 3;
            continue;
        }
        decoded.push(bytes[index]);
        index += 1;
    }
    decoded
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn percent_encode(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(bytes.len());
    for &byte in bytes {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(byte as char);
        } else {
            encoded.push('%');
            encoded.push(
                char::from_digit((byte >> 4) as u32, 16)
                    .unwrap()
                    .to_ascii_uppercase(),
            );
            encoded.push(
                char::from_digit((byte & 0x0f) as u32, 16)
                    .unwrap()
                    .to_ascii_uppercase(),
            );
        }
    }
    encoded
}
