use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use rust_hft::core::FixedPoint8;

fn benchmark_fixed_point_add(c: &mut Criterion) {
    let a = FixedPoint8::from_raw(100_000_000);
    let b = FixedPoint8::from_raw(200_000_000);

    c.bench_function("fixed_point_add", |bench| {
        bench.iter(|| {
            black_box(a.checked_add(b))
        })
    });
}

fn benchmark_fixed_point_mul(c: &mut Criterion) {
    let a = FixedPoint8::from_raw(100_000_000);
    let b = FixedPoint8::from_raw(200_000_000);

    c.bench_function("fixed_point_safe_mul", |bench| {
        bench.iter(|| {
            black_box(a.safe_mul(b))
        })
    });
}

fn benchmark_fixed_point_div(c: &mut Criterion) {
    let a = FixedPoint8::from_raw(600_000_000);
    let b = FixedPoint8::from_raw(200_000_000);

    c.bench_function("fixed_point_safe_div", |bench| {
        bench.iter(|| {
            black_box(a.safe_div(b))
        })
    });
}

fn benchmark_fixed_point_parse(c: &mut Criterion) {
    let input = b"12345.6789";

    c.bench_function("fixed_point_parse_bytes", |bench| {
        bench.iter(|| {
            black_box(FixedPoint8::parse_bytes(input))
        })
    });
}

fn benchmark_fixed_point_write(c: &mut Criterion) {
    let value = FixedPoint8::from_raw(123_456_789_00);
    let mut buf = [0u8; 32];

    c.bench_function("fixed_point_write_to_buffer", |bench| {
        bench.iter(|| {
            black_box(value.write_to_buffer(&mut buf))
        })
    });
}

fn benchmark_spread_calculation(c: &mut Criterion) {
    let bid = FixedPoint8::from_raw(100 * FixedPoint8::SCALE);
    let ask = FixedPoint8::from_raw(101 * FixedPoint8::SCALE);

    c.bench_function("spread_bps_calculation", |bench| {
        bench.iter(|| {
            black_box(bid.spread_bps(ask))
        })
    });
}

fn benchmark_batch_operations(c: &mut Criterion) {
    let values: Vec<FixedPoint8> = (0..1000)
        .map(|i| FixedPoint8::from_raw(i * 1_000_000))
        .collect();

    let mut group = c.benchmark_group("batch_operations");
    group.throughput(Throughput::Elements(1000));

    group.bench_function("sum_1000", |bench| {
        bench.iter(|| {
            let mut sum = FixedPoint8::ZERO;
            for &v in values.iter() {
                if let Some(new_sum) = sum.checked_add(v) {
                    sum = new_sum;
                }
            }
            black_box(sum);
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    benchmark_fixed_point_add,
    benchmark_fixed_point_mul,
    benchmark_fixed_point_div,
    benchmark_fixed_point_parse,
    benchmark_fixed_point_write,
    benchmark_spread_calculation,
    benchmark_batch_operations
);
criterion_main!(benches);
