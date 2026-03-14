#![allow(missing_docs)]

use criterion::{Criterion, criterion_group, criterion_main};
use git_forge_core::metadata::{
    approvals::{Approval, ApprovalKind, NewApproval},
    comments::{Comment, LineRange, NewComment, Reply},
};

fn bench_line_range(c: &mut Criterion) {
    c.bench_function("LineRange::new", |b| {
        b.iter(|| LineRange {
            start: criterion::black_box(1),
            end: criterion::black_box(42),
        })
    });
}

fn bench_approval_kind_as_str(c: &mut Criterion) {
    let kinds = [
        ApprovalKind::Blob,
        ApprovalKind::Tree,
        ApprovalKind::Patch,
        ApprovalKind::Range,
    ];
    c.bench_function("ApprovalKind::equality", |b| {
        b.iter(|| {
            for &a in &kinds {
                for &b_kind in &kinds {
                    criterion::black_box(a == b_kind);
                }
            }
        })
    });
}

fn bench_approval_construction(c: &mut Criterion) {
    c.bench_function("Approval::construct", |b| {
        b.iter(|| Approval {
            object_id: criterion::black_box("abc123".to_owned()),
            approver: criterion::black_box("fingerprint-aabbcc".to_owned()),
            timestamp: criterion::black_box("2024-01-01T00:00:00Z".to_owned()),
            kind: criterion::black_box(ApprovalKind::Patch),
            path: criterion::black_box(None),
            message: criterion::black_box(Some("LGTM".to_owned())),
        })
    });

    c.bench_function("NewApproval::construct", |b| {
        b.iter(|| NewApproval {
            object_id: criterion::black_box("abc123".to_owned()),
            kind: criterion::black_box(ApprovalKind::Range),
            path: criterion::black_box(None),
            message: criterion::black_box(None),
        })
    });
}

fn bench_comment_construction(c: &mut Criterion) {
    c.bench_function("Comment::construct", |b| {
        b.iter(|| Comment {
            id: criterion::black_box("comment-001".to_owned()),
            blob_oid: criterion::black_box(git2::Oid::zero()),
            range: LineRange { start: 10, end: 20 },
            context_lines: criterion::black_box(vec![
                "fn foo() {".to_owned(),
                "    bar();".to_owned(),
                "}".to_owned(),
            ]),
            author: criterion::black_box("fingerprint-001".to_owned()),
            timestamp: criterion::black_box("2024-01-01T00:00:00Z".to_owned()),
            body: criterion::black_box("This needs a doc comment.".to_owned()),
            resolved: criterion::black_box(None),
            replies: criterion::black_box(vec![]),
        })
    });

    c.bench_function("NewComment::construct", |b| {
        b.iter(|| NewComment {
            blob_oid: criterion::black_box(git2::Oid::zero()),
            range: LineRange { start: 1, end: 5 },
            context_lines: criterion::black_box(vec!["use std::fmt;".to_owned()]),
            body: criterion::black_box("Consider deriving Display instead.".to_owned()),
        })
    });
}

fn bench_reply_construction(c: &mut Criterion) {
    c.bench_function("Reply::construct", |b| {
        b.iter(|| Reply {
            index: criterion::black_box("001".to_owned()),
            author: criterion::black_box("fingerprint-002".to_owned()),
            timestamp: criterion::black_box("2024-01-02T00:00:00Z".to_owned()),
            body: criterion::black_box("Agreed, will fix.".to_owned()),
        })
    });
}

criterion_group!(
    benches,
    bench_line_range,
    bench_approval_kind_as_str,
    bench_approval_construction,
    bench_comment_construction,
    bench_reply_construction,
);
criterion_main!(benches);
