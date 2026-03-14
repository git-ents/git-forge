#![allow(missing_docs)]

use criterion::{Criterion, criterion_group, criterion_main};
use git_forge_review::indices::reviews::{
    NewReview, REVIEWS_REF_PREFIX, Review, ReviewMeta, ReviewState, ReviewUpdate, Revision,
};

fn bench_review_state_as_str(c: &mut Criterion) {
    c.bench_function("ReviewState::as_str/open", |b| {
        b.iter(|| criterion::black_box(ReviewState::Open.as_str()))
    });

    c.bench_function("ReviewState::as_str/merged", |b| {
        b.iter(|| criterion::black_box(ReviewState::Merged.as_str()))
    });

    c.bench_function("ReviewState::as_str/closed", |b| {
        b.iter(|| criterion::black_box(ReviewState::Closed.as_str()))
    });
}

fn bench_review_state_equality(c: &mut Criterion) {
    c.bench_function("ReviewState::equality", |b| {
        b.iter(|| {
            criterion::black_box(ReviewState::Open == ReviewState::Open);
            criterion::black_box(ReviewState::Open == ReviewState::Merged);
            criterion::black_box(ReviewState::Merged == ReviewState::Closed);
        })
    });
}

fn bench_review_ref(c: &mut Criterion) {
    c.bench_function("Reviews::review_ref/small_id", |b| {
        b.iter(|| {
            let id: u64 = criterion::black_box(1);
            criterion::black_box(format!("{REVIEWS_REF_PREFIX}{id}"))
        })
    });

    c.bench_function("Reviews::review_ref/large_id", |b| {
        b.iter(|| {
            let id: u64 = criterion::black_box(99_999);
            criterion::black_box(format!("{REVIEWS_REF_PREFIX}{id}"))
        })
    });
}

fn bench_revision_construction(c: &mut Criterion) {
    c.bench_function("Revision::construct", |b| {
        b.iter(|| Revision {
            index: criterion::black_box("001".to_owned()),
            head_commit: criterion::black_box(git2::Oid::zero()),
            timestamp: criterion::black_box("2024-01-01T00:00:00Z".to_owned()),
        })
    });
}

fn bench_new_review_construction(c: &mut Criterion) {
    c.bench_function("NewReview::construct", |b| {
        b.iter(|| NewReview {
            target_branch: criterion::black_box("refs/heads/main".to_owned()),
            description: criterion::black_box("Add criterion benchmarks to each crate.".to_owned()),
            head_commit: criterion::black_box(git2::Oid::zero()),
        })
    });
}

fn bench_review_meta_construction(c: &mut Criterion) {
    c.bench_function("ReviewMeta::construct", |b| {
        b.iter(|| ReviewMeta {
            author: criterion::black_box("fingerprint-aabb".to_owned()),
            target_branch: criterion::black_box("refs/heads/main".to_owned()),
            state: criterion::black_box(ReviewState::Open),
            created: criterion::black_box("2024-01-01T00:00:00Z".to_owned()),
        })
    });
}

fn bench_review_construction(c: &mut Criterion) {
    c.bench_function("Review::construct/no_revisions", |b| {
        b.iter(|| Review {
            id: criterion::black_box(1),
            meta: ReviewMeta {
                author: criterion::black_box("fingerprint-aabb".to_owned()),
                target_branch: criterion::black_box("refs/heads/main".to_owned()),
                state: criterion::black_box(ReviewState::Open),
                created: criterion::black_box("2024-01-01T00:00:00Z".to_owned()),
            },
            description: criterion::black_box("Initial review.".to_owned()),
            revisions: criterion::black_box(vec![]),
        })
    });

    c.bench_function("Review::construct/with_revisions", |b| {
        b.iter(|| Review {
            id: criterion::black_box(7),
            meta: ReviewMeta {
                author: criterion::black_box("fingerprint-ccdd".to_owned()),
                target_branch: criterion::black_box("refs/heads/main".to_owned()),
                state: criterion::black_box(ReviewState::Merged),
                created: criterion::black_box("2024-03-10T08:00:00Z".to_owned()),
            },
            description: criterion::black_box(
                "Refactor the index module and add tests.".to_owned(),
            ),
            revisions: criterion::black_box(vec![
                Revision {
                    index: "001".to_owned(),
                    head_commit: git2::Oid::zero(),
                    timestamp: "2024-03-10T08:00:00Z".to_owned(),
                },
                Revision {
                    index: "002".to_owned(),
                    head_commit: git2::Oid::zero(),
                    timestamp: "2024-03-11T09:30:00Z".to_owned(),
                },
            ]),
        })
    });
}

fn bench_review_update_construction(c: &mut Criterion) {
    c.bench_function("ReviewUpdate::construct/empty", |b| {
        b.iter(|| criterion::black_box(ReviewUpdate::default()))
    });

    c.bench_function("ReviewUpdate::construct/full", |b| {
        b.iter(|| ReviewUpdate {
            description: criterion::black_box(Some("Updated description.".to_owned())),
            state: criterion::black_box(Some(ReviewState::Closed)),
        })
    });
}

criterion_group!(
    benches,
    bench_review_state_as_str,
    bench_review_state_equality,
    bench_review_ref,
    bench_revision_construction,
    bench_new_review_construction,
    bench_review_meta_construction,
    bench_review_construction,
    bench_review_update_construction,
);
criterion_main!(benches);
