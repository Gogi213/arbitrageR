//! Benchmarks for Spread Calculator
//!
//! Target: <50ns per calculation

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rust_hft::core::{FixedPoint8, Symbol, TickerData};
use rust_hft::hot_path::SpreadCalculator;

fn make_ticker(bid: i64, ask: i64) -> TickerData {
    TickerData {
        symbol: Symbol::BTCUSDT,
        bid_price: FixedPoint8::from_raw(bid),
        ask_price: FixedPoint8::from_raw(ask),
        bid_qty: FixedPoint8::ONE,
        ask_qty: FixedPoint8::ONE,
        timestamp: 1000,
    }
}

fn bench_spread_calculation(c: &mut Criterion) {
    let binance = make_ticker(100_000_000, 101_000_000);
    let bybit = make_ticker(102_000_000, 103_000_000);
    let symbol = Symbol::BTCUSDT;

    c.bench_function("spread_calc_hot_path", |b| {
        b.iter(|| {
            SpreadCalculator::calculate(
                black_box(symbol),
                black_box(&binance),
                black_box(&bybit)
            )
        })
    });
}

criterion_group!(benches, bench_spread_calculation);
criterion_main!(benches);
