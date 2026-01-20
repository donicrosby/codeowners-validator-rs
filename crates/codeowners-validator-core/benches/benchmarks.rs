//! Benchmarks for codeowners-validator-core
//!
//! Run with: cargo bench -p codeowners-validator-core --features generate
//!
//! Filter benchmarks:
//!   cargo bench -- "parsing"
//!   cargo bench -- "checks/standard"
//!   cargo bench -- "checks/experimental"

use codeowners_validator_core::parse::parse_codeowners;
use codeowners_validator_core::validate::checks::{
    AvoidShadowingCheck, Check, CheckConfig, CheckContext, DupPatternsCheck, FilesCheck,
    NotOwnedCheck, SyntaxCheck,
};
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};

mod fixtures;
use fixtures::{fixtures, fixtures_extended, repo_path};

/// Benchmark parsing across all fixture sizes
fn bench_parsing(c: &mut Criterion) {
    let mut group = c.benchmark_group("parsing");

    for (name, content) in fixtures() {
        group.throughput(Throughput::Bytes(content.len() as u64));
        group.bench_with_input(
            BenchmarkId::new("parse_codeowners", name),
            content,
            |b, input| b.iter(|| parse_codeowners(std::hint::black_box(input))),
        );
    }
    group.finish();
}

/// Benchmark parsing with extended sizes (up to 3MB)
fn bench_parsing_extended(c: &mut Criterion) {
    let mut group = c.benchmark_group("parsing/extended");
    group.sample_size(10); // Fewer samples for large files

    for (name, content) in fixtures_extended() {
        group.throughput(Throughput::Bytes(content.len() as u64));
        group.bench_with_input(
            BenchmarkId::new("parse_codeowners", name),
            content,
            |b, input| b.iter(|| parse_codeowners(std::hint::black_box(input))),
        );
    }
    group.finish();
}

/// Benchmark standard validation checks
fn bench_standard_checks(c: &mut Criterion) {
    let mut group = c.benchmark_group("checks/standard");
    // Use temp repo with files matching generated patterns
    let repo = repo_path();
    let config = CheckConfig::new();

    for (name, content) in fixtures() {
        let parsed = parse_codeowners(content);
        let ctx = CheckContext::new(&parsed.ast, repo, &config);

        // Benchmark each check individually
        group.bench_with_input(BenchmarkId::new("syntax", name), &ctx, |b, ctx| {
            b.iter(|| SyntaxCheck::new().run(std::hint::black_box(ctx)))
        });

        group.bench_with_input(BenchmarkId::new("duppatterns", name), &ctx, |b, ctx| {
            b.iter(|| DupPatternsCheck::new().run(std::hint::black_box(ctx)))
        });

        group.bench_with_input(BenchmarkId::new("files", name), &ctx, |b, ctx| {
            b.iter(|| FilesCheck::new().run(std::hint::black_box(ctx)))
        });
    }
    group.finish();
}

/// Benchmark experimental validation checks
fn bench_experimental_checks(c: &mut Criterion) {
    let mut group = c.benchmark_group("checks/experimental");
    // Use temp repo with files matching generated patterns
    let repo = repo_path();
    let config = CheckConfig::new();

    for (name, content) in fixtures() {
        let parsed = parse_codeowners(content);
        let ctx = CheckContext::new(&parsed.ast, repo, &config);

        group.bench_with_input(BenchmarkId::new("notowned", name), &ctx, |b, ctx| {
            b.iter(|| NotOwnedCheck::new().run(std::hint::black_box(ctx)))
        });

        group.bench_with_input(BenchmarkId::new("avoid-shadowing", name), &ctx, |b, ctx| {
            b.iter(|| AvoidShadowingCheck::new().run(std::hint::black_box(ctx)))
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_parsing,
    bench_parsing_extended,
    bench_standard_checks,
    bench_experimental_checks
);
criterion_main!(benches);
