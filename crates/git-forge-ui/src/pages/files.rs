use std::{
    collections::HashMap,
    panic::{AssertUnwindSafe, catch_unwind},
};

use gix_forge::{AnnotationState, Binding, Comment, LineRange};
use topcoat::{
    Result,
    context::Cx,
    router::{content::Form, page, query_params},
    view::{Unescaped, View, view},
};

use crate::{
    confirm,
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
    blame: Option<String>,
}

#[page("/tree")]
async fn files(cx: &Cx) -> Result {
    let query = query_params::<TreeQuery>(cx)?;
    let requested_ref = query.reference.clone();
    let requested_path = query.path.clone();
    let view = supported_view(query.view.as_deref());
    let comment_target = CommentTarget::parse(query.comment.as_deref());
    let blame = blame_on(query.blame.as_deref());
    let browse = with_repo(cx, move |repo| {
        tree::load(repo, requested_ref.as_deref(), requested_path.as_deref())
            .map_err(|error| gix_forge::Error::QueryRules(error.to_string()))
    })
    .await;

    let content = match browse {
        Ok(browse) => {
            let refs_pane = refs_pane_view(cx, &browse, view).await?;
            let main = browse_view(cx, &browse, view, comment_target, blame, None).await?;
            (view! { cx =>
                <div class="tree-layout">
                    (refs_pane)
                    <div class="tree-main">(main)</div>
                </div>
            })?
        }
        Err(error) => error_panel(cx, "Could not load repository tree", &error).await?,
    };

    view! { shell(active: Some(Tab::Code), title: "Files", keyword: None, child: content) }
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
    let view = supported_view(input.get("view").map(String::as_str))
        .unwrap_or("source")
        .to_owned();
    let Some(target) = CommentTarget::parse(input.get("comment").map(String::as_str)) else {
        return comment_failure(
            cx,
            &reference,
            &path,
            &view,
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
            &view,
            target,
            "A file path and comment are required.",
        )
        .await;
    }

    // Every mutation routes through the shared confirm dialog before it
    // touches the repository: `confirm_step` renders a screen showing the
    // target pseudo-ref and the signing identity on the first submit, and
    // only returns `None` (letting us fall through to the real write below)
    // once the re-POST carries `confirmed=true`. See `src/confirm.rs`.
    let target_ref = match target {
        CommentTarget::File => "refs/meta/comments/file".to_owned(),
        CommentTarget::Lines { start, end } if start == end => {
            format!("refs/meta/comments/line-{start}")
        }
        CommentTarget::Lines { start, end } => format!("refs/meta/comments/line-{start}-{end}"),
    };
    let cancel_href = tree_href(
        &percent_encode(reference.as_bytes()),
        &encode_query_path(&path),
        Some(&view),
    );
    let summary = (view! { cx =>
        match target {
            CommentTarget::File => {
                "Post a new comment on " <code>(path.as_str())</code>
            },
            CommentTarget::Lines { start, end } if start == end => {
                "Post a new comment on line " (start) " of " <code>(path.as_str())</code>
            },
            CommentTarget::Lines { start, end } => {
                "Post a new comment on lines " (start) "\u{2013}" (end) " of " <code>(path.as_str())</code>
            },
        }
    })?;
    if let Some(confirm) = confirm::confirm_step(
        cx,
        &input,
        "/tree/comments",
        &target_ref,
        &cancel_href,
        summary,
    )
    .await?
    {
        return view! { cx =>
            shell(active: Some(Tab::Code), title: "Confirm comment", keyword: None, child: confirm)
        };
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
                Some(&view),
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
            view! { shell(active: Some(Tab::Code), title: "Comment published", keyword: None, child: content) }
        }
        Err(error) => {
            if let Some(error) = crate::auth::authorization_error(&error) {
                return Err(error);
            }
            comment_failure(cx, &reference, &path, &view, target, &error).await
        }
    }
}

/// The files route's 17rem refs pane: every local branch, sorted by
/// most-recently-active, each with its real ahead-count and age (see
/// `tree::list_refs`). No author-identity grouping or CI status -- the
/// forge has neither, and fabricating either would misrepresent the data.
async fn refs_pane_view(cx: &Cx, browse: &Browse, view: Option<&str>) -> Result {
    let current_selector = browse.reference.selector.clone();
    let refs = with_repo(cx, |repo| {
        tree::list_refs(repo).map_err(|error| gix_forge::Error::QueryRules(error.to_string()))
    })
    .await
    .unwrap_or_default();
    view! { cx =>
        <nav class="refs-pane" aria-label="Branches">
            <h3 class="refs-pane-heading">"Branches"</h3>
            if refs.is_empty() {
                <p class="empty">"No branches found."</p>
            } else {
                <ul class="refs-list">
                    for reference in &refs {
                        let ref_class = if reference.selector == current_selector { "ref-link active" } else { "ref-link" };
                        <li>
                            <a class=(ref_class) href=(tree_href(&reference.selector, "", view))>
                                <span class="ref-name">(reference.display_name.as_str())</span>
                                <span class="ref-meta">
                                    if reference.ahead > 0 {
                                        <span class="ref-ahead">(format!("+{}", reference.ahead))</span>
                                    }
                                    <span class="ref-age">(format_age(reference.last_commit_seconds))</span>
                                </span>
                            </a>
                        </li>
                    }
                </ul>
            }
        </nav>
    }
}

async fn browse_view(
    cx: &Cx,
    browse: &Browse,
    view: Option<&str>,
    comment_target: Option<CommentTarget>,
    blame: bool,
    comment_error: Option<&str>,
) -> Result {
    match &browse.view {
        BrowseView::Directory { entries, .. } => directory_view(cx, browse, entries, view).await,
        BrowseView::File(file) => {
            file_view(cx, browse, file, view, comment_target, blame, comment_error).await
        }
    }
}

async fn directory_view(
    cx: &Cx,
    browse: &Browse,
    entries: &[tree::Entry],
    view: Option<&str>,
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
                    <a href=(tree_href(&browse.reference.selector, &breadcrumb.path, view))>
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
                                    view.or_else(|| default_file_view(&entry.path))
                                } else {
                                    view
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
    view: Option<&str>,
    comment_target: Option<CommentTarget>,
    blame: bool,
    comment_error: Option<&str>,
) -> Result {
    let current_path = browse.path.as_str();
    let effective_view = view
        .or_else(|| default_file_view(current_path))
        .or_else(|| is_previewable(current_path).then_some("preview"))
        .unwrap_or("source");
    let source = effective_view == "source";
    let comment_path = current_path.to_owned();
    let commit_id = browse.reference.commit_id;
    let file_subject = format!("file:{}", file_subject_id(current_path));
    // Fetch every comment anchored to this path at *any* commit, not only
    // the one currently being viewed: computing a real bound/rebound/
    // orphaned confidence (below) requires seeing comments whose anchor was
    // captured elsewhere and may have moved or broken since.
    let comments = with_repo(cx, move |repo| {
        let comments = Comment::list_all(repo)?
            .into_iter()
            .filter(|comment| {
                comment.subject.as_deref() == Some(file_subject.as_str())
                    || matches!(
                        comment.binding.as_ref(),
                        Some(Binding::Position(anchor)) if anchor.identity.path == comment_path
                    )
            })
            .map(|comment| {
                let state = Comment::annotation_state(repo, &comment, commit_id)?;
                Ok((comment, state))
            })
            .collect::<std::result::Result<Vec<_>, gix_forge::Error>>()?;
        Ok(comments)
    })
    .await
    .unwrap_or_default();
    let can_comment = crate::auth::can_create::<Comment>(cx).await;
    let source_bytes = file.text.as_bytes();
    let anchored_comments = comments
        .iter()
        .filter_map(|(comment, state)| {
            anchored_line_range(comment, current_path, source_bytes).map(|range| {
                let confidence = state.map(Confidence::from).unwrap_or(Confidence::Orphaned);
                AnchoredComment {
                    comment: comment.clone(),
                    range,
                    confidence,
                }
            })
        })
        .collect::<Vec<_>>();
    let file_comment_form = if comment_target == Some(CommentTarget::File) && can_comment {
        Some(
            comment_form(
                cx,
                browse,
                CommentTarget::File,
                effective_view,
                comment_error,
                None,
            )
            .await?,
        )
    } else {
        None
    };
    let line_count = if file.text.is_empty() {
        1
    } else {
        file.text.lines().count()
    };
    let language_label = arborium::detect_language(current_path)
        .map(capitalize)
        .unwrap_or_else(|| "Plain text".to_owned());
    let last_commit = with_repo(cx, move |repo| {
        tree::commit_summary(repo, commit_id)
            .map_err(|error| gix_forge::Error::QueryRules(error.to_string()))
    })
    .await
    .ok();
    let file_name = current_path
        .rsplit('/')
        .next()
        .unwrap_or(current_path)
        .to_owned();
    let blame_href = blame_toggle_href(&browse.reference.selector, &browse.route_path, view, blame);
    let blob_header = (view! { cx =>
        <div class="blob-header">
            <span class="blob-name">(file_name.as_str())</span>
            <span class="blob-meta">
                (language_label.as_str()) " · " (line_count) " lines · " (file.size) " B"
            </span>
            if let Some(commit) = &last_commit {
                <span class="blob-meta">
                    "last: " <code class="blob-oid">(commit.short_oid.as_str())</code>
                    " \u{201c}" (commit.subject.as_str()) "\u{201d} \u{00b7} "
                    (commit.author.as_str()) " \u{00b7} " (format_age(commit.seconds))
                </span>
            }
            <span class="blob-meta thread-count">(comments.len()) " threads"</span>
            <div class="blob-actions">
                if source {
                    <a
                        class=(if blame { "blame-toggle active" } else { "blame-toggle" })
                        href=(blame_href)
                    >
                        if blame { "blame: on" } else { "blame" }
                    </a>
                }

            </div>
        </div>
    })?;
    let body: View = if source {
        source_view(
            cx,
            browse,
            file,
            line_count,
            &anchored_comments,
            comment_target,
            comment_error,
            can_comment,
            blame,
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

    // File-level thread cards: whole-file comments (or unanchored subject
    // comments) always show here, and so do orphaned line-anchored comments
    // -- they no longer resolve to any line, so the file panel is the only
    // place left to surface them (previously they were silently dropped).
    let mut file_threads = Vec::new();
    for (comment, state) in comments.iter() {
        let confidence = state.map(Confidence::from);
        let is_whole_file =
            comment.binding.is_none() || is_file_anchor(comment, current_path, source_bytes);
        let is_orphaned = confidence == Some(Confidence::Orphaned);
        if is_whole_file || is_orphaned {
            file_threads.push((view! { cx =>
                <article
                    class=(match confidence {
                        Some(c) => format!("comment file-comment {}", c.css_class()),
                        None => "comment file-comment".to_owned(),
                    })
                    id=(format!("comment-{}", comment.id))
                >
                    <div class="comment-heading">
                        <strong>(comment.author.as_str())</strong>
                        if let Some(confidence) = confidence {
                            <span class=(format!("confidence-label {}", confidence.css_class()))>
                                (confidence.label())
                            </span>
                        }
                        <a class="muted" href=(format!("/comments/{}", comment.id))>"Permalink"</a>
                    </div>
                    <p>(comment.body.as_str())</p>
                </article>
            })?);
        }
    }

    view! { cx =>
        <section class="panel tree-panel">
            <nav class="breadcrumbs" aria-label="Breadcrumb">
                for breadcrumb in &browse.breadcrumbs {
                    <a href=(tree_href(&browse.reference.selector, &breadcrumb.path, view))>
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
            (blob_header)
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
                            effective_view,
                        ))>"Comment on file"</a>
                    }
                </div>
                if let Some(form) = file_comment_form {
                    (form)
                } else if !can_comment {
                    <p class="form-context">"Sign in as a forge member to comment on this file."</p>
                }
                for thread in file_threads { (thread) }
            </section>
            (body)
        </section>
    }
}

#[allow(clippy::too_many_arguments)]
async fn source_view(
    cx: &Cx,
    browse: &Browse,
    file: &File,
    line_count: usize,
    anchored_comments: &[AnchoredComment],
    comment_target: Option<CommentTarget>,
    comment_error: Option<&str>,
    can_comment: bool,
    blame: bool,
) -> Result<View> {
    // The syntax-highlighted HTML is generated once for the whole file and
    // then split at line boundaries -- see `highlighted_source_lines`'s docs
    // for why that's safe to do textually rather than re-highlighting each
    // line (which would also break cross-line constructs like block
    // comments).
    let highlighted_lines = highlighted_source_lines(&browse.path, &file.text, line_count);
    let mut rendered_lines = Vec::with_capacity(line_count);
    let selected_range = comment_target.and_then(|target| match target {
        CommentTarget::Lines { start, end } => Some((start, end)),
        CommentTarget::File => None,
    });
    let selected_start = selected_range.map_or_else(String::new, |(start, _)| start.to_string());
    let selected_end = selected_range.map_or_else(String::new, |(_, end)| end.to_string());
    let source_label = selected_range.map_or_else(
        || "Source code".to_owned(),
        |(start, end)| format!("Source code, comment range lines {start} through {end} selected"),
    );
    for (index, highlighted) in highlighted_lines.into_iter().enumerate() {
        let line_number = index as u64 + 1;
        let composer = if comment_target.is_some_and(|target| target.is_start(line_number)) {
            Some(
                comment_form(
                    cx,
                    browse,
                    comment_target.unwrap_or(CommentTarget::File),
                    "source",
                    comment_error,
                    Some(line_count as u64),
                )
                .await?,
            )
        } else {
            None
        };
        let line_comment_label = match selected_range {
            Some((start, end)) if (start..=end).contains(&line_number) && start != end => {
                format!(
                    "Comment on line {line_number} (selected range: lines {start} through {end})"
                )
            }
            _ => format!("Comment on line {line_number}"),
        };
        let mut line_class = String::from("source-line");
        if comment_target.is_some_and(|target| target.contains(line_number)) {
            line_class.push_str(" selected");
        }
        if blame {
            line_class.push_str(" with-blame");
        }
        let thread_confidence = anchored_comments
            .iter()
            .find(|anchored| anchored.range.start == line_number)
            .map(|anchored| anchored.confidence);
        let dot_class = match thread_confidence {
            Some(confidence) => format!("thread-dot {}", confidence.css_class()),
            None => "thread-dot".to_owned(),
        };
        rendered_lines.push((view! { cx =>
            <li class=(line_class.as_str()) id=(format!("L{line_number}")) data-line=(line_number) data-selected-start=(selected_start.as_str()) data-selected-end=(selected_end.as_str())>
                <a class="line-number" href=(format!("#L{line_number}")) aria-label=(format!("Line {line_number}"))>
                    <span class=(dot_class.as_str()) aria-hidden="true"></span>
                    (line_number)
                </a>
                if blame {
                    // No real per-line blame is available: `gix` 0.85 (this
                    // repo's pinned version) has no `blame` feature/crate in
                    // its dependency tree, so there is no cheap way to
                    // compute it. Rather than fabricate an author per line,
                    // the column renders structurally with a placeholder --
                    // see the "Blame column" section of this crate's
                    // implementation report.
                    <span class="line-blame">"—"</span>
                }
                <code class="line-content">(Unescaped::new_unchecked(highlighted))</code>
                if let Some(form) = composer {
                    (form)
                }
                if can_comment {
                    <div class="line-actions">
                        <a class="line-comment" href=(comment_href(
                            &browse.reference.selector,
                            &browse.route_path,
                            CommentTarget::Lines { start: line_number, end: line_number },
                            "source",
                        )) aria-label=(line_comment_label.as_str())>"Comment"</a>
                    </div>
                }
                for anchored in anchored_comments.iter() {
                    if anchored.range.start == line_number {
                        <article class=(format!("source-comment {}", anchored.confidence.css_class())) id=(format!("comment-{}", anchored.comment.id)) data-comment-start=(anchored.range.start) data-comment-end=(anchored.range.end)>
                            <div class="comment-heading">
                                <strong>(anchored.comment.author.as_str())</strong>
                                <span class=(format!("confidence-label {}", anchored.confidence.css_class()))>
                                    (anchored.confidence.label())
                                </span>
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
        <div class="source-view" tabindex="0" aria-label=(source_label.as_str()) data-source-path=(browse.path.as_str())>
            <ol class="source-lines" aria-label=(source_label.as_str()) data-selected-start=(selected_start.as_str()) data-selected-end=(selected_end.as_str())>
                for line in rendered_lines { (line) }
            </ol>
        </div>
    }
}

async fn comment_form(
    cx: &Cx,
    browse: &Browse,
    target: CommentTarget,
    view: &str,
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
            <input type="hidden" name="view" value=(view)>
            <input type="hidden" name="comment" value=(target_name.as_str())>
            <label for=(format!("file-comment-body-{}", target_name))>
                if let CommentTarget::File = target { "Comment on this file" } else { "Comment on selected lines" }
            </label>
            if let CommentTarget::Lines { start, end } = target {
                <p class="form-context">"Selected lines: " (start) "–" (end)</p>
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
            <textarea id=(format!("file-comment-body-{}", target_name)) name="body" rows="4" placeholder="Leave a helpful note." required="" autofocus=""></textarea>
            <button type="submit">"Comment"</button>
        </form>
    }
}

async fn comment_failure(
    cx: &Cx,
    reference: &str,
    path: &str,
    view: &str,
    target: CommentTarget,
    error: &str,
) -> Result {
    let href = comment_href(
        &percent_encode(reference.as_bytes()),
        &encode_query_path(path),
        target,
        view,
    );
    let content = (view! { cx =>
        <div class="error-panel">
            <strong>"Comment could not be saved"</strong>
            <p>(error)</p>
            <a href=(href)>"Return to comment composer"</a>
        </div>
    })?;
    view! { cx => shell(active: Some(Tab::Code), title: "File comment", keyword: None, child: content) }
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

/// A thread's anchor-confidence state, per HANDOFF.md packet B: encodes how
/// much to trust that a comment still applies to where it visually sits.
///
/// Computed from data already in scope, not mocked:
/// - `Bound`: the anchor's captured-at commit (`binding_genesis`) is exactly
///   the commit currently being viewed.
/// - `Rebound`: the captured-at commit differs, but the anchor still
///   resolves to a line range (or the whole file) in the current text --
///   moved, but still applies.
/// - `Orphaned`: the anchor no longer resolves against current content at
///   all.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Confidence {
    Bound,
    Drifted,
    Orphaned,
    Pinned,
}

impl Confidence {
    pub(crate) fn css_class(self) -> &'static str {
        match self {
            Self::Bound => "confidence-bound",
            Self::Drifted => "confidence-drifted",
            Self::Orphaned => "confidence-orphaned",
            Self::Pinned => "confidence-pinned",
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Bound => "bound here",
            Self::Drifted => "drifted",
            Self::Orphaned => "orphaned",
            Self::Pinned => "pinned",
        }
    }
}

impl From<AnnotationState> for Confidence {
    fn from(state: AnnotationState) -> Self {
        match state {
            AnnotationState::Bound => Self::Bound,
            AnnotationState::Drifted => Self::Drifted,
            AnnotationState::Orphaned => Self::Orphaned,
            AnnotationState::Pinned => Self::Pinned,
        }
    }
}

struct AnchoredComment {
    comment: Comment,
    range: LineRange,
    confidence: Confidence,
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

fn comment_href(reference: &str, route_path: &str, target: CommentTarget, view: &str) -> String {
    let mut href = tree_href(reference, route_path, Some(view));
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

fn supported_view(view: Option<&str>) -> Option<&'static str> {
    match view {
        Some("source") => Some("source"),
        Some("preview") => Some("preview"),
        _ => None,
    }
}

fn blame_on(value: Option<&str>) -> bool {
    matches!(value, Some(value) if value == "1" || value.eq_ignore_ascii_case("true"))
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

/// A link that adds (or removes) the `blame=1` query parameter while
/// preserving the ref, path, and view -- see the "Blame column" section of
/// this crate's implementation report for what the toggle does and does not
/// power.
fn blame_toggle_href(reference: &str, route_path: &str, view: Option<&str>, blame: bool) -> String {
    let mut href = tree_href(reference, route_path, view);
    if !blame {
        href.push_str("&blame=1");
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

fn capitalize(value: &str) -> String {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// Format a Unix timestamp as a short, human "age" string (`"3d ago"`,
/// `"40m ago"`, ...), used for both the refs pane and the blob header's last
/// commit line.
fn format_age(seconds: i64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(seconds);
    let delta = (now - seconds).max(0);
    const MINUTE: i64 = 60;
    const HOUR: i64 = 60 * MINUTE;
    const DAY: i64 = 24 * HOUR;
    const WEEK: i64 = 7 * DAY;
    const MONTH: i64 = 30 * DAY;
    const YEAR: i64 = 365 * DAY;
    if delta < MINUTE {
        "just now".to_owned()
    } else if delta < HOUR {
        format!("{}m ago", delta / MINUTE)
    } else if delta < DAY {
        format!("{}h ago", delta / HOUR)
    } else if delta < WEEK {
        format!("{}d ago", delta / DAY)
    } else if delta < MONTH {
        format!("{}w ago", delta / WEEK)
    } else if delta < YEAR {
        format!("{}mo ago", delta / MONTH)
    } else {
        format!("{}y ago", delta / YEAR)
    }
}

/// Split `highlight_file`'s whole-document HTML into one HTML fragment per
/// source line, so each line can live in its own `<li>` for the sticky
/// gutter.
///
/// `arborium`'s `ClassNamesWithPrefix` renderer never nests `<span>` tags --
/// at most one span wraps any given run of text -- so a run that crosses a
/// line break can simply be closed at the break and reopened on the next
/// line; this is the same technique terminal/editor line-splitters use, and
/// it keeps every line's fragment independently valid HTML.
///
/// `expected_lines` is the caller's own line count (from splitting the raw
/// source text). `arborium` trims trailing newlines from the source before
/// highlighting, which can make its output end one or more blank lines
/// short of the caller's count, so the result is padded with empty lines to
/// match.
fn highlighted_source_lines(path: &str, source: &str, expected_lines: usize) -> Vec<String> {
    if source.is_empty() {
        return vec![String::new(); expected_lines.max(1)];
    }
    // `highlight_file` falls back to `ammonia::clean_text` when there's no
    // detected (or supported) grammar for `path`, and unlike arborium's own
    // escaper, `clean_text` encodes `\n` as the `&#10;` entity rather than
    // leaving it literal (it's built for arbitrary untrusted attribute
    // text, not specifically for code). It's still correct HTML either
    // way, but this scanner splits lines by looking for a literal `\n`, so
    // decode that one entity back before scanning.
    let html = highlight_file(path, source).replace("&#10;", "\n");
    let mut lines = Vec::with_capacity(expected_lines);
    let mut current = String::new();
    let mut open_class: Option<String> = None;
    let mut rest = html.as_str();
    const OPEN_PREFIX: &str = "<span class=\"";
    const CLOSE_TAG: &str = "</span>";
    loop {
        let next_open = rest.find(OPEN_PREFIX);
        let next_close = rest.find(CLOSE_TAG);
        let boundary = match (next_open, next_close) {
            (Some(open), Some(close)) => Some((open.min(close), open <= close)),
            (Some(open), None) => Some((open, true)),
            (None, Some(close)) => Some((close, false)),
            (None, None) => None,
        };
        let Some((index, is_open)) = boundary else {
            push_highlighted_text(&mut lines, &mut current, open_class.as_deref(), rest);
            break;
        };
        let (text, remainder) = rest.split_at(index);
        push_highlighted_text(&mut lines, &mut current, open_class.as_deref(), text);
        if is_open {
            let after = &remainder[OPEN_PREFIX.len()..];
            let Some(quote_end) = after.find('"') else {
                push_highlighted_text(&mut lines, &mut current, open_class.as_deref(), remainder);
                break;
            };
            if open_class.is_some() {
                current.push_str(CLOSE_TAG);
            }
            let class_name = &after[..quote_end];
            current.push_str(OPEN_PREFIX);
            current.push_str(class_name);
            current.push_str("\">");
            open_class = Some(class_name.to_owned());
            rest = &after[quote_end + 2..];
        } else {
            if open_class.take().is_some() {
                current.push_str(CLOSE_TAG);
            }
            rest = &remainder[CLOSE_TAG.len()..];
        }
    }
    lines.push(current);
    while lines.len() < expected_lines {
        lines.push(String::new());
    }
    lines.truncate(expected_lines);
    lines
}

/// Append `text` to `current`, splitting it into `lines` at every `\n` and
/// carrying `open_class` (if any) across the break by closing it before the
/// break and reopening it after.
fn push_highlighted_text(
    lines: &mut Vec<String>,
    current: &mut String,
    open_class: Option<&str>,
    text: &str,
) {
    let mut parts = text.split('\n');
    if let Some(first) = parts.next() {
        current.push_str(first);
    }
    for part in parts {
        if open_class.is_some() {
            current.push_str("</span>");
        }
        lines.push(std::mem::take(current));
        if let Some(class_name) = open_class {
            current.push_str("<span class=\"");
            current.push_str(class_name);
            current.push_str("\">");
        }
        current.push_str(part);
    }
}

#[cfg(test)]
mod tests {
    use super::highlighted_source_lines;

    #[test]
    fn splits_highlighted_html_by_line_without_leaking_open_tags() {
        let source = "fn a() {}\nfn b() {}\n";
        let lines = highlighted_source_lines("example.rs", source, 2);
        assert_eq!(lines.len(), 2);
        for line in &lines {
            assert_eq!(
                line.matches("<span").count(),
                line.matches("</span>").count()
            );
        }
        assert!(lines[0].contains('a'));
        assert!(lines[1].contains('b'));
    }

    #[test]
    fn carries_a_multi_line_span_across_the_break() {
        let source = "/* one\ntwo */\nfn c() {}\n";
        let lines = highlighted_source_lines("example.rs", source, 3);
        assert_eq!(lines.len(), 3);
        for line in &lines {
            assert_eq!(
                line.matches("<span").count(),
                line.matches("</span>").count()
            );
        }
        assert!(lines[0].contains("one"));
        assert!(lines[1].contains("two"));
    }

    #[test]
    fn pads_trailing_blank_lines_arborium_trims() {
        let source = "fn a() {}\n\n\n";
        let lines = highlighted_source_lines("example.rs", source, 3);
        assert_eq!(lines.len(), 3);
        assert!(lines[1].is_empty() || lines[1].trim().is_empty());
        assert!(lines[2].is_empty() || lines[2].trim().is_empty());
    }

    #[test]
    fn handles_plain_text_with_no_detected_language() {
        let source = "hello\nworld\n";
        let lines = highlighted_source_lines("notes.txt", source, 2);
        assert_eq!(lines, vec!["hello".to_owned(), "world".to_owned()]);
    }

    #[test]
    fn handles_empty_file() {
        let lines = highlighted_source_lines("empty.rs", "", 1);
        assert_eq!(lines, vec![String::new()]);
    }
}
