//! Benchmarks for Threshold Tracker
//!
//! Target: <100ns per update

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rust_hft::core::{FixedPoint8, Symbol, TickerData};
use rust_hft::exchanges::Exchange;
use rust_hft::hot_path::ThresholdTracker;

fn make_ticker(symbol: Symbol, price: i64) -> TickerData {
    TickerData {
        symbol,
        bid_price: FixedPoint8::from_raw(price),
        ask_price: FixedPoint8::from_raw(price + 100),
        bid_qty: FixedPoint8::ONE,
        ask_qty: FixedPoint8::ONE,
        timestamp: 1000,
    }
}

fn bench_tracker_update(c: &mut Criterion) {
    let mut tracker = ThresholdTracker::new();
    let symbol = Symbol::BTCUSDT;
    let ticker = make_ticker(symbol, 100_000_000);
    
    // Warmup
    tracker.update(ticker, Exchange::Binance);
    
    c.bench_function("tracker_update", |b| {
        b.iter(|| {
            // Alternate between exchanges to trigger calculation
            tracker.update(black_box(ticker), black_box(Exchange::Binance));
            tracker.update(black_box(ticker), black_box(Exchange::Bybit));
        })
    });
}

criterion_group!(benches, bench_tracker_update);
criterion_main!(benches);
