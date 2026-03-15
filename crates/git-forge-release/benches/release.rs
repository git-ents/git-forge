#![allow(missing_docs)]

use criterion::{Criterion, criterion_group, criterion_main};
use git_forge_release::cli::ReleaseCommand;

fn bench_release_command_variants(c: &mut Criterion) {
    c.bench_function("ReleaseCommand::New", |b| {
        b.iter(|| criterion::black_box(ReleaseCommand::New));
    });

    c.bench_function("ReleaseCommand::Edit", |b| {
        b.iter(|| criterion::black_box(ReleaseCommand::Edit));
    });

    c.bench_function("ReleaseCommand::List", |b| {
        b.iter(|| criterion::black_box(ReleaseCommand::List));
    });

    c.bench_function("ReleaseCommand::Status", |b| {
        b.iter(|| criterion::black_box(ReleaseCommand::Status));
    });

    c.bench_function("ReleaseCommand::Show", |b| {
        b.iter(|| criterion::black_box(ReleaseCommand::Show));
    });
}

criterion_group!(benches, bench_release_command_variants);
criterion_main!(benches);
