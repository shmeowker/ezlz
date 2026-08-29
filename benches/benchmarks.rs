use criterion::{Criterion, criterion_group, criterion_main};
use ezlz::t;
use std::hint::black_box;

use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

fn benchmarks(c: &mut Criterion) {
    ezlz::init("test", "tests/locales").unwrap();

    let x: u32 = 100;
    let xf: f32 = 100.0;
    let name = "Anna".to_string();

    let mut group = c.benchmark_group("ezlz");

    group.bench_function("text", |b| {
        b.iter(|| black_box(t!("test", bench.literal)));
    });

    group.bench_function("simple", |b| {
        b.iter(|| black_box(t!("test", bench.simple, x)));
    });

    group.bench_function("simple<-string", |b| {
        b.iter(|| black_box(t!("test", bench.simple, x = name)));
    });

    group.bench_function("simple<-float", |b| {
        b.iter(|| black_box(t!("test", bench.simple, x = xf)));
    });

    group.bench_function("simple (x10)", |b| {
        b.iter(|| black_box(t!("test", bench.simple_x10, x)));
    });

    group.bench_function("plural_en", |b| {
        b.iter(|| black_box(t!("test", bench.plural_en, x)));
    });

    group.bench_function("plural_en<-float", |b| {
        b.iter(|| black_box(t!("test", bench.plural_en, x = xf)));
    });

    group.bench_function("plural_en (x10)", |b| {
        b.iter(|| black_box(t!("test", bench.plural_en_x10, x)));
    });

    group.bench_function("plural_fr<-float", |b| {
        b.iter(|| black_box(t!("test", bench.plural_fr, x = xf)));
    });

    group.bench_function("plural_ru", |b| {
        b.iter(|| black_box(t!("test", bench.plural_ru, x)));
    });

    group.bench_function("plural_ru<-float", |b| {
        b.iter(|| black_box(t!("test", bench.plural_ru, x = xf)));
    });

    group.finish();
}

criterion_group!(benches, benchmarks);
criterion_main!(benches);
