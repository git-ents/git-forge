use std::{
    collections::HashMap,
    panic::{AssertUnwindSafe, catch_unwind},
};

use gix_forge::{Binding, Comment, LineRange, binding_genesis};
use topcoat::{
    Result,
    context::Cx,
    router::{content::Form, page, query_params},
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
    comment: Option<String>,
}

#[page("/tree")]
async fn files(cx: &Cx) -> Result {
    let query = query_params::<TreeQuery>(cx)?;
    let requested_ref = query.reference.clone();
    let requested_path = query.path.clone();
    let source = query.view.as_deref() == Some("source");
    let comment_target = CommentTarget::parse(query.comment.as_deref());
    let browse = with_repo(cx, move |repo| {
        tree::load(repo, requested_ref.as_deref(), requested_path.as_deref())
            .map_err(|error| gix_forge::Error::QueryRules(error.to_string()))
    })
    .await;

    let content = match browse {
        Ok(browse) => browse_view(cx, &browse, source, comment_target, None).await?,
        Err(error) => error_panel(cx, "Could not load repository tree", &error).await?,
    };

    view! { shell(active: Tab::Files, title: "Files", keyword: None, child: content) }
}

#[page(POST "/tree/comments")]
async fn file_comment_create(cx: &Cx, Form(input): Form<HashMap<String, String>>) -> Result {
    let authorization = crate::auth::authorization(cx).await?;
    let reference = input
        .get("reference")
        .map(String::as_str)
        .filter(|reference| !reference.is_empty())
        .unwrap_or("HEAD")
        .to_owned();
    let path = input
        .get("path")
        .map(String::as_str)
        .unwrap_or_default()
        .to_owned();
    let Some(target) = CommentTarget::parse(input.get("comment").map(String::as_str)) else {
        return comment_failure(
            cx,
            &reference,
            &path,
            CommentTarget::File,
            "Choose a file or line range.",
        )
        .await;
    };
    let target = match target {
        CommentTarget::Lines { start, end } => input
            .get("end")
            .and_then(|value| value.parse().ok())
            .filter(|candidate| *candidate >= start)
            .map_or(CommentTarget::Lines { start, end }, |end| {
                CommentTarget::Lines { start, end }
            }),
        target => target,
    };
    let body = input
        .get("body")
        .map(String::as_str)
        .unwrap_or_default()
        .trim()
        .to_owned();
    if path.is_empty() || body.is_empty() {
        return comment_failure(
            cx,
            &reference,
            &path,
            target,
            "A file path and comment are required.",
        )
        .await;
    }

    let anchor_path = path.clone();
    let anchor_reference = reference.clone();
    let result = with_repo(cx, move |repo| {
        let lines = match target {
            CommentTarget::File => None,
            CommentTarget::Lines { start, end } => Some(LineRange { start, end }),
        };
        Comment::create_anchored_in_repo_as(
            repo,
            &authorization,
            &anchor_reference,
            &anchor_path,
            lines,
            &body,
        )
    })
    .await;

    match result {
        Ok(_) => {
            let href = tree_href(
                &percent_encode(reference.as_bytes()),
                &encode_query_path(&path),
                Some("source"),
            );
            let content = (view! {
                <section class="success-panel">
                    <span class="success-icon">"✓"</span>
                    <p class="eyebrow">"PUBLISHED"</p>
                    <h2>"Your file comment is live"</h2>
                    <p class="muted">"The source view now includes the comment anchor."</p>
                    <a class="button-link" href=(href)>"Return to file"</a>
                </section>
            })?;
            view! { shell(active: Tab::Files, title: "Comment published", keyword: None, child: content) }
        }
        Err(error) => {
            if let Some(error) = crate::auth::authorization_error(&error) {
                return Err(error);
            }
            comment_failure(cx, &reference, &path, target, &error).await
        }
    }
}

async fn browse_view(
    cx: &Cx,
    browse: &Browse,
    source: bool,
    comment_target: Option<CommentTarget>,
    comment_error: Option<&str>,
) -> Result {
    match &browse.view {
        BrowseView::Directory { entries, .. } => directory_view(cx, browse, entries).await,
        BrowseView::File(file) => {
            file_view(cx, browse, file, source, comment_target, comment_error).await
        }
    }
}

async fn directory_view(cx: &Cx, browse: &Browse, entries: &[tree::Entry]) -> Result {
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
            </div>
            if entries.is_empty() {
                <p class="empty">"This directory is empty."</p>
            } else {
                <ul class="entity-list file-list">
                    for entry in entries {
                        <li>
                            <a class="entity-link" href=(tree_href(
                                &browse.reference.selector,
                                &entry.path,
                                if entry.kind == gix::objs::tree::EntryKind::Blob
                                    || entry.kind == gix::objs::tree::EntryKind::BlobExecutable
                                {
                                    default_file_view(&entry.path)
                                } else {
                                    None
                                },
                            ))>
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
                                </span>
                            </a>
                        </li>
                    }
                </ul>
            }
        </section>
    }
}

async fn file_view(
    cx: &Cx,
    browse: &Browse,
    file: &File,
    source: bool,
    comment_target: Option<CommentTarget>,
    comment_error: Option<&str>,
) -> Result {
    let current_path = browse.path.as_str();
    let comment_path = current_path.to_owned();
    let commit_id = browse.reference.commit_id;
    let file_subject = format!("file:{}", file_subject_id(current_path));
    let comments = with_repo(cx, move |repo| {
        Ok(Comment::list_all(repo)?
            .into_iter()
            .filter(|comment| {
                comment.subject.as_deref() == Some(file_subject.as_str())
                    || matches!(
                        comment.binding.as_ref(),
                        Some(binding @ Binding::Position(anchor))
                            if anchor.identity.path == comment_path
                                && binding_genesis(binding) == Some(commit_id)
                    )
            })
            .collect::<Vec<_>>())
    })
    .await
    .unwrap_or_default();
    let can_comment = crate::auth::can_create::<Comment>(cx).await;
    let anchored_comments = comments
        .iter()
        .filter_map(|comment| {
            anchored_line_range(comment, current_path, file.text.as_bytes()).map(|range| {
                AnchoredComment {
                    comment: comment.clone(),
                    range,
                }
            })
        })
        .collect::<Vec<_>>();
    let file_comment_form = if comment_target == Some(CommentTarget::File) && can_comment {
        Some(comment_form(cx, browse, CommentTarget::File, comment_error, None).await?)
    } else {
        None
    };
    let body: View = if source {
        source_view(
            cx,
            browse,
            file,
            &anchored_comments,
            comment_target,
            comment_error,
            can_comment,
        )
        .await?
    } else if is_previewable(current_path) {
        match render_preview(current_path, &file.text) {
            Ok(Preview::RenderedHtml(html)) => {
                (view! { cx =>
                    <div class="rendered-file">(Unescaped::new_unchecked(html))</div>
                })?
            }
            Ok(Preview::CodeHtml(html)) => {
                (view! { cx =>
                    <div class="code-view">(Unescaped::new_unchecked(html))</div>
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
    } else {
        (view! { cx =>
            <pre class="source-view"><code>(file.text.as_str())</code></pre>
        })?
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
            </div>
            <div class="view-toggle" aria-label="File view">
                if is_previewable(current_path) {
                    <a href=(tree_href(&browse.reference.selector, &browse.route_path, Some("preview")))>
                        "Preview"
                    </a>
                }
                <a href=(tree_href(&browse.reference.selector, &browse.route_path, Some("source")))>
                    "Source"
                </a>
            </div>
            <section class="file-comments" aria-label="File comments">
                <div class="panel-heading">
                    <div>
                        <h3>"File comments"</h3>
                        <span class="muted">"Notes attached to this file"</span>
                    </div>
                    if can_comment && comment_target != Some(CommentTarget::File) {
                        <a class="button-link secondary" href=(comment_href(
                            &browse.reference.selector,
                            &browse.route_path,
                            CommentTarget::File,
                        ))>"Comment on file"</a>
                    }
                </div>
                if let Some(form) = file_comment_form {
                    (form)
                } else if !can_comment {
                    <p class="form-context">"Sign in as a forge member to comment on this file."</p>
                }
                for comment in comments.iter() {
                    if comment.binding.is_none()
                        || is_file_anchor(comment, current_path, file.text.as_bytes()) {
                        <article class="comment file-comment" id=(format!("comment-{}", comment.id))>
                            <div class="comment-heading">
                                <strong>(comment.author.as_str())</strong>
                                <a class="muted" href=(format!("/comments/{}", comment.id))>"Permalink"</a>
                            </div>
                            <p>(comment.body.as_str())</p>
                        </article>
                    }
                }
            </section>
            (body)
        </section>
    }
}

async fn source_view(
    cx: &Cx,
    browse: &Browse,
    file: &File,
    anchored_comments: &[AnchoredComment],
    comment_target: Option<CommentTarget>,
    comment_error: Option<&str>,
    can_comment: bool,
) -> Result<View> {
    let lines = if file.text.is_empty() {
        vec![""]
    } else {
        file.text.split_inclusive('\n').collect::<Vec<_>>()
    };
    let mut rendered_lines = Vec::with_capacity(lines.len());
    for (index, line) in lines.iter().enumerate() {
        let line_number = index as u64 + 1;
        let composer = if comment_target.is_some_and(|target| target.is_start(line_number)) {
            Some(
                comment_form(
                    cx,
                    browse,
                    comment_target.unwrap_or(CommentTarget::File),
                    comment_error,
                    Some(lines.len() as u64),
                )
                .await?,
            )
        } else {
            None
        };
        rendered_lines.push((view! { cx =>
            <li class=(if comment_target.is_some_and(|target| target.contains(line_number)) { "source-line selected" } else { "source-line" }) id=(format!("L{line_number}")) data-line=(line_number)>
                <a class="line-number" href=(format!("#L{line_number}")) aria-label=(format!("Line {line_number}"))>(line_number)</a>
                <code class="line-content">(line)</code>
                if let Some(form) = composer {
                    (form)
                }
                if can_comment {
                    <div class="line-actions">
                        <a class="line-comment" href=(comment_href(
                            &browse.reference.selector,
                            &browse.route_path,
                            CommentTarget::Lines { start: line_number, end: line_number },
                        ))>"Comment"</a>
                    </div>
                }
                for anchored in anchored_comments.iter() {
                    if anchored.range.start == line_number {
                        <article class="source-comment" id=(format!("comment-{}", anchored.comment.id)) data-comment-start=(anchored.range.start) data-comment-end=(anchored.range.end)>
                            <div class="comment-heading">
                                <strong>(anchored.comment.author.as_str())</strong>
                                <a class="muted" href=(format!("/comments/{}", anchored.comment.id))>"Permalink"</a>
                            </div>
                            <p class="comment-range">"Lines " (anchored.range.start) "–" (anchored.range.end)</p>
                            <p>(anchored.comment.body.as_str())</p>
                        </article>
                    }
                }
            </li>
        })?);
    }
    view! { cx =>
        <div class="source-view" data-source-path=(browse.path.as_str())>
            <ol class="source-lines" aria-label="Source code">
                for line in rendered_lines { (line) }
            </ol>
        </div>
    }
}

async fn comment_form(
    cx: &Cx,
    browse: &Browse,
    target: CommentTarget,
    error: Option<&str>,
    line_count: Option<u64>,
) -> Result<View> {
    let reference = browse.reference.name.as_str();
    let target_name = target.query();
    let max_line = match target {
        CommentTarget::Lines { end, .. } => line_count.unwrap_or(end),
        CommentTarget::File => 0,
    };
    view! { cx =>
        <form class="comment-form inline-comment-form" action="/tree/comments" method="post" data-comment-target=(target_name.as_str())>
            <input type="hidden" name="reference" value=(reference)>
            <input type="hidden" name="path" value=(browse.path.as_str())>
            <input type="hidden" name="comment" value=(target_name.as_str())>
            <label for=(format!("file-comment-body-{}", target_name))>
                if let CommentTarget::File = target { "Comment on this file" } else { "Comment on selected lines" }
            </label>
            if let CommentTarget::Lines { start, end } = target {
                <p class="form-context">"Starting at line " (start)</p>
                <label for=(format!("file-comment-end-{}", target_name))>"Through line"</label>
                <select id=(format!("file-comment-end-{}", target_name)) name="end">
                    for end_line in start..=max_line {
                        if end_line == end {
                            <option value=(end_line) selected="">(end_line)</option>
                        } else {
                            <option value=(end_line)>(end_line)</option>
                        }
                    }
                </select>
            }
            if let Some(error) = error {
                <div class="error-panel"><strong>"Comment could not be saved"</strong><p>(error)</p></div>
            }
            <textarea id=(format!("file-comment-body-{}", target_name)) name="body" rows="4" placeholder="Leave a helpful note." required=""></textarea>
            <button type="submit">"Comment"</button>
        </form>
    }
}

async fn comment_failure(
    cx: &Cx,
    reference: &str,
    path: &str,
    target: CommentTarget,
    error: &str,
) -> Result {
    let href = comment_href(
        &percent_encode(reference.as_bytes()),
        &encode_query_path(path),
        target,
    );
    let content = (view! { cx =>
        <div class="error-panel">
            <strong>"Comment could not be saved"</strong>
            <p>(error)</p>
            <a href=(href)>"Return to comment composer"</a>
        </div>
    })?;
    view! { cx => shell(active: Tab::Files, title: "File comment", keyword: None, child: content) }
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum CommentTarget {
    File,
    Lines { start: u64, end: u64 },
}

impl CommentTarget {
    fn parse(value: Option<&str>) -> Option<Self> {
        let value = value?;
        if value == "file" {
            return Some(Self::File);
        }
        let (start, end) = value.split_once('-').unwrap_or((value, value));
        let start = start.parse().ok()?;
        let end = end.parse().ok()?;
        (start > 0 && end >= start).then_some(Self::Lines { start, end })
    }

    fn query(self) -> String {
        match self {
            Self::File => "file".to_owned(),
            Self::Lines { start, end } => format!("{start}-{end}"),
        }
    }

    fn contains(self, line: u64) -> bool {
        match self {
            Self::File => false,
            Self::Lines { start, end } => (start..=end).contains(&line),
        }
    }

    fn is_start(self, line: u64) -> bool {
        matches!(self, Self::Lines { start, .. } if start == line)
    }
}

struct AnchoredComment {
    comment: Comment,
    range: LineRange,
}

fn file_subject_id(path: &str) -> String {
    percent_encode(path.as_bytes())
}

fn anchored_line_range(comment: &Comment, path: &str, source: &[u8]) -> Option<LineRange> {
    if is_file_anchor(comment, path, source) {
        return None;
    }
    let Some(Binding::Position(anchor)) = comment.binding.as_ref() else {
        return None;
    };
    if anchor.identity.path != path {
        return None;
    }
    let start = usize::try_from(anchor.identity.span.start).ok()?;
    let end = usize::try_from(anchor.identity.span.end).ok()?;
    if start > end || end > source.len() {
        return None;
    }
    let end_offset = if start == end { start } else { end - 1 };
    Some(LineRange {
        start: line_at_offset(source, start)?,
        end: line_at_offset(source, end_offset)?,
    })
}

fn is_file_anchor(comment: &Comment, path: &str, source: &[u8]) -> bool {
    let Some(Binding::Position(anchor)) = comment.binding.as_ref() else {
        return false;
    };
    anchor.identity.path == path
        && anchor.identity.span.start == 0
        && anchor.identity.span.end == source.len() as u64
}

fn line_at_offset(source: &[u8], offset: usize) -> Option<u64> {
    let source = source.get(..offset)?;
    Some(1 + source.iter().filter(|byte| **byte == b'\n').count() as u64)
}

fn comment_href(reference: &str, route_path: &str, target: CommentTarget) -> String {
    let mut href = tree_href(reference, route_path, Some("source"));
    href.push_str("&comment=");
    href.push_str(&target.query());
    if let CommentTarget::Lines { start, .. } = target {
        href.push_str("#L");
        href.push_str(&start.to_string());
    }
    href
}

enum Preview {
    RenderedHtml(String),
    CodeHtml(String),
    Source(String),
}

fn render_preview(path: &str, source: &str) -> std::result::Result<Preview, String> {
    catch_unwind(AssertUnwindSafe(|| {
        if is_markdown(path) {
            Preview::RenderedHtml(render_markdown(source))
        } else if is_asciidoc(path) {
            Preview::RenderedHtml(render_asciidoc(source))
        } else if arborium::detect_language(path).is_some() {
            Preview::CodeHtml(highlight_file(path, source))
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

fn is_previewable(path: &str) -> bool {
    is_markdown(path) || is_asciidoc(path)
}

fn default_file_view(path: &str) -> Option<&'static str> {
    is_readme(path).then_some("preview")
}

fn is_readme(path: &str) -> bool {
    let Some(name) = path.rsplit('/').next() else {
        return false;
    };
    let Some((stem, _)) = name.rsplit_once('.') else {
        return false;
    };

    stem.eq_ignore_ascii_case("README") && is_markdown(path)
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
