#![allow(missing_docs)]

use criterion::{Criterion, criterion_group, criterion_main};
use git_forge_issues::issues::{
    ISSUES_REF_PREFIX, Issue, IssueMeta, IssueState, IssueUpdate, NewIssue,
};

fn bench_issue_state_as_str(c: &mut Criterion) {
    c.bench_function("IssueState::as_str/open", |b| {
        b.iter(|| criterion::black_box(IssueState::Open.as_str()))
    });

    c.bench_function("IssueState::as_str/closed", |b| {
        b.iter(|| criterion::black_box(IssueState::Closed.as_str()))
    });
}

fn bench_issue_state_equality(c: &mut Criterion) {
    c.bench_function("IssueState::equality", |b| {
        b.iter(|| {
            criterion::black_box(IssueState::Open == IssueState::Open);
            criterion::black_box(IssueState::Open == IssueState::Closed);
        })
    });
}

fn bench_issue_ref(c: &mut Criterion) {
    c.bench_function("Issues::issue_ref/small_id", |b| {
        b.iter(|| {
            let id: u64 = criterion::black_box(1);
            criterion::black_box(format!("{ISSUES_REF_PREFIX}{id}"))
        })
    });

    c.bench_function("Issues::issue_ref/large_id", |b| {
        b.iter(|| {
            let id: u64 = criterion::black_box(99_999);
            criterion::black_box(format!("{ISSUES_REF_PREFIX}{id}"))
        })
    });
}

fn bench_new_issue_construction(c: &mut Criterion) {
    c.bench_function("NewIssue::construct/no_labels_no_assignees", |b| {
        b.iter(|| NewIssue {
            title: criterion::black_box("Fix the thing".to_owned()),
            body: criterion::black_box("Detailed description here.".to_owned()),
            labels: criterion::black_box(vec![]),
            assignees: criterion::black_box(vec![]),
        })
    });

    c.bench_function("NewIssue::construct/with_labels_and_assignees", |b| {
        b.iter(|| NewIssue {
            title: criterion::black_box("Refactor module layout".to_owned()),
            body: criterion::black_box("We should reorganize the crate structure.".to_owned()),
            labels: criterion::black_box(vec![
                "refactor".to_owned(),
                "good-first-issue".to_owned(),
            ]),
            assignees: criterion::black_box(vec![
                "fingerprint-aabb".to_owned(),
                "fingerprint-ccdd".to_owned(),
            ]),
        })
    });
}

fn bench_issue_meta_construction(c: &mut Criterion) {
    c.bench_function("IssueMeta::construct", |b| {
        b.iter(|| IssueMeta {
            author: criterion::black_box("fingerprint-0011".to_owned()),
            title: criterion::black_box("Add benchmarks".to_owned()),
            state: criterion::black_box(IssueState::Open),
            labels: criterion::black_box(vec!["perf".to_owned()]),
            assignees: criterion::black_box(vec![]),
            created: criterion::black_box("2024-01-01T00:00:00Z".to_owned()),
        })
    });
}

fn bench_issue_construction(c: &mut Criterion) {
    c.bench_function("Issue::construct/no_comments", |b| {
        b.iter(|| Issue {
            id: criterion::black_box(1),
            meta: IssueMeta {
                author: criterion::black_box("fingerprint-0011".to_owned()),
                title: criterion::black_box("Add benchmarks".to_owned()),
                state: criterion::black_box(IssueState::Open),
                labels: criterion::black_box(vec![]),
                assignees: criterion::black_box(vec![]),
                created: criterion::black_box("2024-01-01T00:00:00Z".to_owned()),
            },
            body: criterion::black_box("Please add criterion benches.".to_owned()),
            comments: criterion::black_box(vec![]),
        })
    });

    c.bench_function("Issue::construct/with_comments", |b| {
        b.iter(|| Issue {
            id: criterion::black_box(42),
            meta: IssueMeta {
                author: criterion::black_box("fingerprint-0011".to_owned()),
                title: criterion::black_box("Discuss approach".to_owned()),
                state: criterion::black_box(IssueState::Closed),
                labels: criterion::black_box(vec!["discussion".to_owned()]),
                assignees: criterion::black_box(vec!["fingerprint-aabb".to_owned()]),
                created: criterion::black_box("2024-03-15T09:00:00Z".to_owned()),
            },
            body: criterion::black_box("What approach should we take?".to_owned()),
            comments: criterion::black_box(vec![
                (
                    "001-2024-03-15T10:00:00Z-fingerprint-aabb".to_owned(),
                    "I think we should go with option A.".to_owned(),
                ),
                (
                    "002-2024-03-15T11:00:00Z-fingerprint-0011".to_owned(),
                    "Agreed, let's proceed.".to_owned(),
                ),
            ]),
        })
    });
}

fn bench_issue_update_construction(c: &mut Criterion) {
    c.bench_function("IssueUpdate::construct/empty", |b| {
        b.iter(|| criterion::black_box(IssueUpdate::default()))
    });

    c.bench_function("IssueUpdate::construct/full", |b| {
        b.iter(|| IssueUpdate {
            title: criterion::black_box(Some("Updated title".to_owned())),
            body: criterion::black_box(Some("Updated body.".to_owned())),
            labels: criterion::black_box(Some(vec!["bug".to_owned(), "urgent".to_owned()])),
            assignees: criterion::black_box(Some(vec!["fingerprint-aabb".to_owned()])),
            state: criterion::black_box(Some(IssueState::Closed)),
        })
    });
}

criterion_group!(
    benches,
    bench_issue_state_as_str,
    bench_issue_state_equality,
    bench_issue_ref,
    bench_new_issue_construction,
    bench_issue_meta_construction,
    bench_issue_construction,
    bench_issue_update_construction,
);
criterion_main!(benches);
