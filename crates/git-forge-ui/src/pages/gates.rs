//! Gates (Handoff v5): CI/merge-gate state derived by declarative rules
//! against `refs/meta/rules`, independent from Releases (README's route
//! list, item 6). There is no gate, rule, or CI concept anywhere in
//! `gix-forge` today -- a search of `crates/gix-forge/src` for "gate" or
//! "rule" turns up zero hits -- so this route has nothing real to query.
//! It previews the intended shape instead, the same way the former
//! `workingcopy.rs` (now `local.rs`) previewed drift/gate predictions
//! before any of that existed: the sample gates below are clearly-labeled
//! `const` data, never framed as a live evaluation.
//!
//! The one real touch: `merge-gate`'s subject reads the repository's
//! actual `HEAD` oid, the same way `local.rs`'s `head_oid` does, rather
//! than inventing one. Everything else per gate (verdict, derivation
//! trace, rule metadata, cache state) is sample content. `release_signable`
//! is named to match Releases' "see release_signable in Gates for whether
//! this can ship" cross-link (README item 7).
//!
//! Both actions on the detail pane -- the per-gate primary action and
//! "Promote result to claim" -- route through the shared [`confirm_step`]
//! and are genuine no-op previews: no rule is evaluated, no ref is
//! written, no claim is signed.

use std::collections::HashMap;

use topcoat::{
    Result,
    context::Cx,
    router::{content::Form, page, path_param},
    view::view,
};

use crate::{
    auth::authorization,
    confirm::confirm_step,
    pages::with_repo,
    shell::{Tab, route_shell, shell},
};

#[path_param(error = bad_request)]
struct GateId(String);

/// Reused for the "Result destination" panel's "keyed on ... engine vN"
/// line -- the one honest version number available, same as `local.rs`.
const ENGINE_VERSION: &str = env!("CARGO_PKG_VERSION");

/// One line of a Datalog-ish derivation trace.
struct TraceStep {
    /// Tree-branch glyph; empty for the root conclusion.
    glyph: &'static str,
    predicate: &'static str,
    value: &'static str,
    /// CSS color, e.g. `"var(--ok)"` -- matches this step's own outcome,
    /// which need not match the gate's overall verdict color.
    value_color: &'static str,
    note: &'static str,
}

/// What a gate's subject is. `Head` is the one real value in this file;
/// `Fixed` is sample content.
enum Subject {
    Head,
    Fixed(&'static str),
}

struct Gate {
    id: &'static str,
    name: &'static str,
    subject: Subject,
    verdict: &'static str,
    /// `"ok"`, `"warn"`, or `"danger"` -- the `.status`/`.tag` modifier
    /// this gate's verdict maps to. Never `"accent"`: accent is brand/nav/
    /// primary-action only, per the non-negotiable color rule.
    status_class: &'static str,
    /// The same semantic color as an inline CSS value (`"var(--ok)"`, ...),
    /// used for the nav row's `--nav-edge` and the trace's own root value.
    color_var: &'static str,
    evaluated: &'static str,
    rule_ref: &'static str,
    rule_tree: &'static str,
    rule_author_age: &'static str,
    cache_ref: &'static str,
    cache_state: &'static str,
    fix_label: &'static str,
    trace: &'static [TraceStep],
}

const MERGE_GATE_TRACE: &[TraceStep] = &[
    TraceStep {
        glyph: "",
        predicate: "merge_gate(HEAD)",
        value: "true",
        value_color: "var(--ok)",
        note: "all conjuncts below held",
    },
    TraceStep {
        glyph: "├─",
        predicate: "required_checks_pass(HEAD)",
        value: "true",
        value_color: "var(--ok)",
        note: "4/4 checks green",
    },
    TraceStep {
        glyph: "├─",
        predicate: "reviews_satisfied(HEAD)",
        value: "true",
        value_color: "var(--ok)",
        note: "2 approvals ≥ quorum 2",
    },
    TraceStep {
        glyph: "└─",
        predicate: "no_conflicts(HEAD, main)",
        value: "true",
        value_color: "var(--ok)",
        note: "fast-forwards cleanly",
    },
];

const RELEASE_SIGNABLE_TRACE: &[TraceStep] = &[
    TraceStep {
        glyph: "",
        predicate: "release_signable(v1.4.0)",
        value: "false",
        value_color: "var(--bad)",
        note: "one conjunct below failed",
    },
    TraceStep {
        glyph: "├─",
        predicate: "quorum_met(v1.4.0)",
        value: "false",
        value_color: "var(--bad)",
        note: "1 of 2 required signatures present",
    },
    TraceStep {
        glyph: "├─",
        predicate: "tag_object_valid(v1.4.0)",
        value: "true",
        value_color: "var(--ok)",
        note: "annotated tag resolves cleanly",
    },
    TraceStep {
        glyph: "└─",
        predicate: "no_revoked_signers(v1.4.0)",
        value: "true",
        value_color: "var(--ok)",
        note: "no revoked keys among current signers",
    },
];

const DOCS_COVERAGE_TRACE: &[TraceStep] = &[
    TraceStep {
        glyph: "",
        predicate: "docs_coverage(tree)",
        value: "pending",
        value_color: "var(--warn)",
        note: "waiting on an upstream goal",
    },
    TraceStep {
        glyph: "└─",
        predicate: "changelog_updated(tree)",
        value: "unknown",
        value_color: "var(--faint)",
        note: "engine has not evaluated this goal yet",
    },
];

const GATES: &[Gate] = &[
    Gate {
        id: "merge-gate",
        name: "merge-gate",
        subject: Subject::Head,
        verdict: "would pass",
        status_class: "ok",
        color_var: "var(--ok)",
        evaluated: "2 minutes ago",
        rule_ref: "refs/meta/rules/merge-gate",
        rule_tree: "8f2c19e4a730b6c1d905e2f4b8a71c0d3e5f6789",
        rule_author_age: "priya · 6 days ago",
        cache_ref: "refs/forge/cache/gates/merge-gate",
        cache_state: "valid",
        fix_label: "Merge now",
        trace: MERGE_GATE_TRACE,
    },
    Gate {
        id: "release_signable",
        name: "release_signable",
        subject: Subject::Fixed("v1.4.0"),
        verdict: "would fail",
        status_class: "danger",
        color_var: "var(--bad)",
        evaluated: "just now",
        rule_ref: "refs/meta/rules/release_signable",
        rule_tree: "3aa914f0c2b7e485a1d6f902c8e3b4a7d0159fce",
        rule_author_age: "priya · 6 days ago",
        cache_ref: "refs/forge/cache/gates/release_signable",
        cache_state: "stale -- input ref moved since last evaluation",
        fix_label: "Request missing signature",
        trace: RELEASE_SIGNABLE_TRACE,
    },
    Gate {
        id: "docs-coverage",
        name: "docs-coverage",
        subject: Subject::Fixed("9c31d5a2e871f04b6c2a9d3e5f70b81c4d926fa3"),
        verdict: "pending",
        status_class: "warn",
        color_var: "var(--warn)",
        evaluated: "queued",
        rule_ref: "refs/meta/rules/docs-coverage",
        rule_tree: "5b60d217e9a4c380f1276e5d9a0b3c7148fe2d61",
        rule_author_age: "priya · 6 days ago",
        cache_ref: "refs/forge/cache/gates/docs-coverage",
        cache_state: "not yet computed",
        fix_label: "Re-run evaluation",
        trace: DOCS_COVERAGE_TRACE,
    },
];

fn find_gate(id: &str) -> Option<&'static Gate> {
    GATES.iter().find(|gate| gate.id == id)
}

/// `merge-gate`'s subject is the repository's real `HEAD` oid (mirrors
/// `local.rs::head_oid`); every other gate's subject is sample text.
async fn subject_text(cx: &Cx, gate: &Gate) -> String {
    match gate.subject {
        Subject::Head => with_repo(cx, |repo| {
            Ok(repo
                .head_id()
                .map(|id| id.to_string())
                .unwrap_or_else(|_| "unborn".to_owned()))
        })
        .await
        .unwrap_or_else(|_| "unknown".to_owned()),
        Subject::Fixed(text) => text.to_owned(),
    }
}

async fn gates_nav(cx: &Cx, selected: &Gate) -> Result {
    view! { cx =>
        <div class="route-nav-header">
            <h2>"Gates"</h2>
            <p class="route-nav-note">"derived by rules in refs/meta/rules — never stored"</p>
        </div>
        for gate in GATES {
            let subject = subject_text(cx, gate).await;
            let is_active = gate.id == selected.id;
            let class = if is_active { "nav-row active" } else { "nav-row" };
            let edge = format!("--nav-edge:{}", gate.color_var);
            <a class=(class) href=(format!("/gates/{}", gate.id)) style=(edge)>
                <div class="nav-row-top">
                    <span class="nav-row-dot"></span>
                    <span class="nav-row-title">(gate.name)</span>
                </div>
                <span class="nav-row-sub">(subject.as_str())</span>
                <span class="nav-row-verdict">(gate.verdict)</span>
            </a>
        }
    }
}

async fn gate_detail(cx: &Cx, gate: &Gate) -> Result {
    let subject = subject_text(cx, gate).await;
    let pill_class = format!("status {} gate-pill", gate.status_class);
    view! { cx =>
        <div class="route-main-width-lg">
            <div class="gate-heading">
                <span class=(pill_class.as_str())>(gate.verdict)</span>
                <h1>(gate.name)</h1>
            </div>
            <p class="gate-subline">
                "subject " <code>(subject.as_str())</code> " · evaluated " (gate.evaluated)
            </p>

            <p class="eyebrow">"DERIVATION"</p>
            <div class="gate-trace">
                for step in gate.trace {
                    <div class="gate-trace-row">
                        <span class="gate-trace-glyph">(step.glyph)</span>
                        <span class="gate-trace-pred">(step.predicate)</span>
                        <span class="gate-trace-arrow">"⟸"</span>
                        <span class="gate-trace-value" style=(format!("color:{}", step.value_color))>(step.value)</span>
                        <span class="gate-trace-note">(step.note)</span>
                    </div>
                }
            </div>

            <div class="gate-info-grid">
                <div class="gate-info-panel">
                    <h3>"Rule"</h3>
                    <dl class="facts">
                        <dt>"Ref"</dt><dd><code class="oid">(gate.rule_ref)</code></dd>
                        <dt>"Tree"</dt><dd><code class="oid">(gate.rule_tree)</code></dd>
                        <dt>"Author"</dt><dd>(gate.rule_author_age)</dd>
                    </dl>
                    <a class="gate-info-link" href="/tree">"open rule for review →"</a>
                </div>
                <div class="gate-info-panel">
                    <h3>"Result destination"</h3>
                    <dl class="facts">
                        <dt>"Memoized"</dt><dd>(gate.cache_state)</dd>
                        <dt>"Cache ref"</dt><dd><code class="oid">(gate.cache_ref)</code></dd>
                        <dt>"Keyed on"</dt><dd>"rule tree + input refs + engine v" (ENGINE_VERSION)</dd>
                    </dl>
                </div>
            </div>

            <div class="gate-actions">
                <form class="inline-form" action=(format!("/gates/{}/fix", gate.id)) method="post">
                    <button type="submit">(gate.fix_label)</button>
                </form>
                <form class="inline-form" action=(format!("/gates/{}/promote", gate.id)) method="post">
                    <button class="button-link secondary" type="submit">"Promote result to claim"</button>
                </form>
            </div>
            <p class="action-note">"promotion is the only door from derived to authoritative"</p>
        </div>
    }
}

async fn render(cx: &Cx, selected_id: &str) -> Result {
    let gate = find_gate(selected_id).unwrap_or(&GATES[0]);
    let nav = gates_nav(cx, gate).await?;
    let detail = gate_detail(cx, gate).await?;
    view! { route_shell(active: Tab::Gates, title: "Gates", nav_width: "19rem", nav: nav, detail: detail) }
}

#[page("/gates")]
async fn gates(cx: &Cx) -> Result {
    let selected = GATES.first().map(|gate| gate.id).unwrap_or_default();
    render(cx, selected).await
}

#[page("/gates/{gate_id}")]
async fn gate_page(cx: &Cx) -> Result {
    let id = path_param::<GateId>(cx)?.clone();
    render(cx, id.as_str()).await
}

/// Preview-only per-gate primary action (e.g. "Merge now", "Request
/// missing signature", "Re-run evaluation" -- see [`Gate::fix_label`]).
/// Routes through [`confirm_step`] like every other mutating POST in this
/// app; there is no rule engine to actually invoke, so confirming performs
/// no mutation.
#[page(POST "/gates/{gate_id}/fix")]
async fn gate_fix(cx: &Cx, Form(input): Form<HashMap<String, String>>) -> Result {
    authorization(cx).await?;
    let id = path_param::<GateId>(cx)?.clone();
    let gate = find_gate(id.as_str()).unwrap_or(&GATES[0]);
    let summary = (view! { cx =>
        "Preview only: there is no rule-evaluation backend yet, so confirming here would trigger \""
        (gate.fix_label) "\" for " <code>(gate.name)</code> " if it were real."
    })?;
    if let Some(confirm) = confirm_step(
        cx,
        &input,
        &format!("/gates/{}/fix", gate.id),
        &format!("refs/meta/rules/{}", gate.id),
        &format!("/gates/{}", gate.id),
        summary,
    )
    .await?
    {
        return view! { shell(active: Some(Tab::Gates), title: "Confirm gate action", keyword: None, child: confirm) };
    }
    let content = (view! { cx =>
        <section class="success-panel">
            <span class="success-icon">"✓"</span>
            <p class="eyebrow">"PREVIEW"</p>
            <h2>(gate.fix_label) " (no-op)"</h2>
            <p class="muted">"This UI has no rule-evaluation backend yet, so nothing was actually re-evaluated, merged, or requested."</p>
            <a class="button-link" href=(format!("/gates/{}", gate.id))>"Back to " (gate.name)</a>
        </section>
    })?;
    view! { shell(active: Some(Tab::Gates), title: "Gate action (preview)", keyword: None, child: content) }
}

/// Preview-only "Promote result to claim". Routes through [`confirm_step`];
/// there is no derived-result store or claim-signing backend to promote
/// into, so confirming performs no mutation.
#[page(POST "/gates/{gate_id}/promote")]
async fn gate_promote(cx: &Cx, Form(input): Form<HashMap<String, String>>) -> Result {
    authorization(cx).await?;
    let id = path_param::<GateId>(cx)?.clone();
    let gate = find_gate(id.as_str()).unwrap_or(&GATES[0]);
    let summary = (view! { cx =>
        "Preview only: promotion is the only door from derived to authoritative, but there is no claim-signing "
        "backend yet, so confirming here would sign and record a claim from " <code>(gate.cache_ref)</code>
        " if it were real."
    })?;
    if let Some(confirm) = confirm_step(
        cx,
        &input,
        &format!("/gates/{}/promote", gate.id),
        &format!("refs/meta/claims/gates/{}", gate.id),
        &format!("/gates/{}", gate.id),
        summary,
    )
    .await?
    {
        return view! { shell(active: Some(Tab::Gates), title: "Confirm promotion", keyword: None, child: confirm) };
    }
    let content = (view! { cx =>
        <section class="success-panel">
            <span class="success-icon">"✓"</span>
            <p class="eyebrow">"PREVIEW"</p>
            <h2>"Promotion confirmed (no-op)"</h2>
            <p class="muted">"This UI has no claim-signing backend yet, so nothing was actually signed or recorded."</p>
            <a class="button-link" href=(format!("/gates/{}", gate.id))>"Back to " (gate.name)</a>
        </section>
    })?;
    view! { shell(active: Some(Tab::Gates), title: "Promotion (preview)", keyword: None, child: content) }
}
