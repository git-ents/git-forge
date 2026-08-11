//! Working copy route (packet E).
//!
//! There is no dirty-worktree, push, signing, or CI/gate-prediction backend
//! in this codebase yet -- that scope was explicitly deferred. This whole
//! board is UI-only sample content previewing the intended design; "Push
//! now" is a no-op that only demonstrates the shared confirm step, it does
//! not read, sign, or push anything real.
//!
//! Real dirty-file detection is available in newer `gix` builds through its
//! worktree status API, but that API sits behind the `status` feature,
//! which pulls in `gix-status`/`gix-dir` as new dependencies not already in
//! this workspace's `Cargo.lock` (checked: neither crate is present today).
//! Wiring that up -- plus computing real per-file added/deleted line counts,
//! which the status iterator doesn't hand back for free -- is more than the
//! "cheap nice-to-have" this packet asked for, so all three columns below
//! are sample data. The real touch used elsewhere in this packet (release
//! tag identity) stays scoped to `releases.rs`.

use std::collections::HashMap;

use topcoat::{
    Result,
    context::Cx,
    router::{content::Form, page},
    view::view,
};

use crate::{
    auth::authorization,
    confirm::confirm_step,
    pages::with_repo,
    shell::{Tab, shell},
};

struct DirtyFile {
    mark: &'static str,
    css_class: &'static str,
    path: &'static str,
    add: u32,
    del: u32,
}

struct UnpushedCommit {
    oid: &'static str,
    subject: &'static str,
    age: &'static str,
    signed: bool,
}

struct GatePrediction {
    name: &'static str,
    pass: bool,
    detail: &'static str,
}

const DIRTY_FILES: [DirtyFile; 3] = [
    DirtyFile {
        mark: "M",
        css_class: "warn",
        path: "src/pages/files.rs",
        add: 42,
        del: 11,
    },
    DirtyFile {
        mark: "M",
        css_class: "warn",
        path: "src/render.rs",
        add: 8,
        del: 3,
    },
    DirtyFile {
        mark: "?",
        css_class: "muted",
        path: "src/pages/blame.rs",
        add: 120,
        del: 0,
    },
];

const UNPUSHED_COMMITS: [UnpushedCommit; 2] = [
    UnpushedCommit {
        oid: "6e4c4b2",
        subject: "files: interleave comment rows into the blob table",
        age: "3h ago",
        signed: true,
    },
    UnpushedCommit {
        oid: "2a4a4df",
        subject: "wip: blame gutter",
        age: "40m ago",
        signed: false,
    },
];

const GATES: [GatePrediction; 3] = [
    GatePrediction {
        name: "fmt-check",
        pass: true,
        detail: "last run 1h ago · 4s",
    },
    GatePrediction {
        name: "unsigned-commit gate",
        pass: false,
        detail: "2a4a4df has no signature -- push will sign it",
    },
    GatePrediction {
        name: "nightly-gate",
        pass: true,
        detail: "last run 6h ago · 3m 12s",
    },
];

async fn real_branch(cx: &Cx) -> String {
    with_repo(cx, |repo| {
        Ok(repo
            .head_name()
            .ok()
            .flatten()
            .map(|name| name.shorten().to_string())
            .unwrap_or_else(|| "detached".to_owned()))
    })
    .await
    .unwrap_or_else(|_| "unknown".to_owned())
}

#[page("/workingcopy")]
async fn workingcopy(cx: &Cx) -> Result {
    let dirty_count = DIRTY_FILES.len();
    let commit_count = UNPUSHED_COMMITS.len();
    let failing_gates = GATES.iter().filter(|gate| !gate.pass).count();
    let unsigned_count = UNPUSHED_COMMITS
        .iter()
        .filter(|commit| !commit.signed)
        .count();
    let push_label = if unsigned_count == 1 {
        "Push now -- signs 1 commit".to_owned()
    } else {
        format!("Push now -- signs {unsigned_count} commits")
    };

    let content = (view! { cx =>
        <p class="muted wc-intro">"Preflight of your next push -- not a file browser. Sample data: there is no dirty-worktree, push, signing, or gate backend wired up yet."</p>
        <p class="wc-summary">
            <span class="wc-count accent">(commit_count) " commits"</span>
            <span class="muted"> " · " </span>
            <span class="wc-count warn">(dirty_count) " dirty files"</span>
            <span class="muted"> " · " </span>
            <span class="wc-count danger">(failing_gates) " gate would fail"</span>
        </p>
        <div class="wc-board">
            <section class="wc-column">
                <h3 class="wc-column-heading">"Uncommitted"</h3>
                <ul class="wc-list">
                    for file in DIRTY_FILES {
                        <li class="wc-dirty-row">
                            <span class=(format!("wc-mark {}", file.css_class))>(file.mark)</span>
                            <span class="wc-path">(file.path)</span>
                            <span class="diffstat"><span class="add">"+" (file.add)</span> " " <span class="del">"−" (file.del)</span></span>
                        </li>
                    }
                </ul>
            </section>

            <section class="wc-column">
                <h3 class="wc-column-heading">"Unpushed commits"</h3>
                <ul class="wc-list">
                    for commit in UNPUSHED_COMMITS {
                        <li class="wc-commit-row">
                            <div class="wc-commit-top">
                                <code class="oid">(commit.oid)</code>
                                <span class="wc-subject">(commit.subject)</span>
                            </div>
                            <div class="wc-commit-meta">
                                <span class="muted">(commit.age)</span>
                                if commit.signed {
                                    <span class="status ok">"signed"</span>
                                } else {
                                    <span class="status danger">"unsigned"</span>
                                }
                            </div>
                        </li>
                    }
                </ul>
            </section>

            <section class="wc-column">
                <h3 class="wc-column-heading">"Gate predictions"</h3>
                <ul class="wc-list">
                    for gate in GATES {
                        <li class="wc-gate-row">
                            <div class="wc-gate-top">
                                <span>(gate.name)</span>
                                if gate.pass {
                                    <span class="status ok">"would pass"</span>
                                } else {
                                    <span class="status danger">"would fail"</span>
                                }
                            </div>
                            <div class="muted wc-gate-detail">(gate.detail)</div>
                        </li>
                    }
                </ul>
                <form class="wc-push" action="/workingcopy/push" method="post">
                    <button type="submit">(push_label)</button>
                </form>
            </section>
        </div>
    })?;
    view! { shell(active: Some(Tab::WorkingCopy), title: "Working copy", keyword: None, child: content) }
}

#[page(POST "/workingcopy/push")]
async fn workingcopy_push(cx: &Cx, Form(input): Form<HashMap<String, String>>) -> Result {
    authorization(cx).await?;
    let branch = real_branch(cx).await;
    let unsigned_count = UNPUSHED_COMMITS
        .iter()
        .filter(|commit| !commit.signed)
        .count();
    let summary = (view! { cx =>
        "Preview only: this records nothing. There is no push or signing backend yet, so confirming here would sign and push "
        (unsigned_count) " sample commit(s) to " <code>(branch.as_str())</code>
        " if it were real."
    })?;
    if let Some(confirm) = confirm_step(
        cx,
        &input,
        "/workingcopy/push",
        &format!("refs/heads/{branch}"),
        "/workingcopy",
        summary,
    )
    .await?
    {
        return view! { shell(active: Some(Tab::WorkingCopy), title: "Confirm push", keyword: None, child: confirm) };
    }
    let content = (view! { cx =>
        <section class="success-panel">
            <span class="success-icon">"✓"</span>
            <p class="eyebrow">"PREVIEW"</p>
            <h2>"Push confirmed (no-op)"</h2>
            <p class="muted">"This UI has no push or signing backend yet, so nothing was actually signed or pushed to " <code>(branch.as_str())</code> "."</p>
            <a class="button-link" href="/workingcopy">"Back to working copy"</a>
        </section>
    })?;
    view! { shell(active: Some(Tab::WorkingCopy), title: "Push (preview)", keyword: None, child: content) }
}
