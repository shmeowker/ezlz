use criterion::{Criterion, criterion_group, criterion_main};
use ezlz::t;
use std::hint::black_box;

fn benchmarks(c: &mut Criterion) {
    // Make sure the localization files are initialized before
    // measuring anything.
    ezlz::init("en", "tests/locales").unwrap();

    let n = 5;
    let name = "Anna";

    let mut group = c.benchmark_group("translation");

    group.bench_function("complex", |b| {
        let n = 5;
        let name = "Anna";
        let place = "inventory";

        b.iter(|| black_box(t!("en", complex, n = n, name = name, place = place)));
    });

    group.bench_function("simple", |b| {
        b.iter(|| black_box(t!("en", messages.hello)));
    });

    group.bench_function("named_placeholder", |b| {
        b.iter(|| black_box(t!("en", messages.greet, name = name)));
    });

    group.bench_function("plural", |b| {
        b.iter(|| black_box(t!("en", plurals.russian, n = n)));
    });

    group.bench_function("plural_threshold", |b| {
        b.iter(|| black_box(t!("en", plurals.items, n = 10)));
    });

    group.bench_function("plural_replacement", |b| {
        b.iter(|| black_box(t!("en", plurals.some, n = 10)));
    });

    group.bench_function("plural_prepend", |b| {
        b.iter(|| black_box(t!("en", plurals.before, n = 10)));
    });

    group.finish();
}

criterion_group!(benches, benchmarks);
criterion_main!(benches);
