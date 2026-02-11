//! Benchmarks for message parsing
//!
//! Target: <500ns per message parse

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};

// Import the parsing functions directly
use rust_hft::exchanges::parsing::{BinanceParser, BybitParser};

// Test data - real exchange message formats
const BINANCE_AGG_TRADE: &[u8] = br#"{"e":"aggTrade","E":1672304484973,"s":"BTCUSDT","a":12345,"p":"25000.50","q":"0.001","f":12340,"l":12344,"T":1672304484972,"m":true}"#;

const BINANCE_BOOK_TICKER: &[u8] = br#"{"e":"bookTicker","u":400900217,"s":"BTCUSDT","b":"25000.50","B":"1.5","a":"25001.00","A":"2.0"}"#;

const BYBIT_PUBLIC_TRADE: &[u8] = br#"{"topic":"publicTrade.BTCUSDT","type":"snapshot","ts":1672304484973,"data":[{"T":1672304484972,"s":"BTCUSDT","S":"Buy","v":"0.001","p":"16500.50","i":"13414134131","BT":false}]}"#;

const BYBIT_TICKERS: &[u8] = br#"{"topic":"tickers.BTCUSDT","type":"snapshot","ts":1672304484973,"data":{"symbol":"BTCUSDT","bid1Price":"25000.50","bid1Size":"1.5","ask1Price":"25001.00","ask1Size":"2.0"}}"#;

fn bench_binance_trade_parse(c: &mut Criterion) {
    let mut group = c.benchmark_group("binance_parse");
    group.throughput(Throughput::Bytes(BINANCE_AGG_TRADE.len() as u64));
    
    group.bench_function("aggTrade", |b| {
        b.iter(|| {
            let result = BinanceParser::parse_trade(black_box(BINANCE_AGG_TRADE));
            black_box(result);
        })
    });
    
    group.finish();
}

fn bench_binance_ticker_parse(c: &mut Criterion) {
    let mut group = c.benchmark_group("binance_parse");
    group.throughput(Throughput::Bytes(BINANCE_BOOK_TICKER.len() as u64));
    
    group.bench_function("bookTicker", |b| {
        b.iter(|| {
            let result = BinanceParser::parse_ticker(black_box(BINANCE_BOOK_TICKER));
            black_box(result);
        })
    });
    
    group.finish();
}

fn bench_bybit_trade_parse(c: &mut Criterion) {
    let mut group = c.benchmark_group("bybit_parse");
    group.throughput(Throughput::Bytes(BYBIT_PUBLIC_TRADE.len() as u64));
    
    group.bench_function("publicTrade", |b| {
        b.iter(|| {
            let result = BybitParser::parse_public_trade(black_box(BYBIT_PUBLIC_TRADE));
            black_box(result);
        })
    });
    
    group.finish();
}

fn bench_bybit_ticker_parse(c: &mut Criterion) {
    let mut group = c.benchmark_group("bybit_parse");
    group.throughput(Throughput::Bytes(BYBIT_TICKERS.len() as u64));
    
    group.bench_function("tickers", |b| {
        b.iter(|| {
            let result = BybitParser::parse_ticker(black_box(BYBIT_TICKERS));
            black_box(result);
        })
    });
    
    group.finish();
}

fn bench_detection(c: &mut Criterion) {
    let mut group = c.benchmark_group("message_detection");
    
    group.bench_function("binance_detect", |b| {
        b.iter(|| {
            let result = BinanceParser::detect_message_type(black_box(BINANCE_AGG_TRADE));
            black_box(result);
        })
    });
    
    group.bench_function("bybit_detect", |b| {
        b.iter(|| {
            let result = BybitParser::detect_message_type(black_box(BYBIT_PUBLIC_TRADE));
            black_box(result);
        })
    });
    
    group.finish();
}

criterion_group!(
    benches,
    bench_binance_trade_parse,
    bench_binance_ticker_parse,
    bench_bybit_trade_parse,
    bench_bybit_ticker_parse,
    bench_detection
);

criterion_main!(benches);
