use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn benchmark_websocket(c: &mut Criterion) {
    // Placeholder benchmark
    c.bench_function("websocket_placeholder", |b| {
        b.iter(|| {
            black_box(42)
        })
    });
}

criterion_group!(benches, benchmark_websocket);
criterion_main!(benches);
